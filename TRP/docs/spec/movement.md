# Movement

How the player moves, written from a reading of the game (0.16.9.5, SPT 4.1.6 assembly). Class and field names are the game's, given so the source can be found again. Items marked **unread** have not been studied yet.

## The shape of it

Movement is driven by the body animator's root motion.

1. Input and inertia produce two animator parameters: `Speed` (0 to 1) and a direction (`Direct_X`, `Direct_Y`).
2. Together with `Level` (stance height), `SprintSpeed` and others, those select and blend locomotion clips.
3. The blended clips' root motion is the displacement for the frame.
4. The active movement state post-processes that displacement and hands it to the character controller.

The movement states (`IdlePlayerState`, `MovePlayerState`, `SprintPlayerState`, `JumpPlayerState`, ...) are attached to states of the body animator. The animator's own transitions decide which movement state is active; the code steers it with flags such as `Inert`, `Sprint`, `IsJumping`, `Landing`. How the animator works, which of its states name which movement state, and what else the body's animation gives (the eye's height, footsteps, where the weapon is carried) are in `body-animator.md`. This document is the formulas the movement states apply on top.

Per frame, for the active state (`MovementState.ManualAnimatorMoveUpdate`):

1. `ProcessRotation`: turn the body toward the look yaw.
2. `ProcessAnimatorMovement`: take the animator's delta position, then `ApplyMotion`:
   1. `BlendMotion` (state-specific; see Idle).
   2. `ProjectMotionToSurface`: on slopes, redirect the motion along the surface.
   3. `ApplyGravity`.
   4. `LimitMotion`: only pushes the motion out of obstacles. It does not clamp speed.
   5. `CharacterController.Move`.

`MovementContext.ManualUpdate` runs once per frame before that: ground check, stance smoothing, pre-sprint acceleration, speed limit expiry, lean smoothing.

**The player's clock.** In a raid the frame time handed to all of this is not the engine's. The game world reads a stopwatch once per frame in whole milliseconds, and the difference from the last reading (at most 1 s) is the player's frame time (`HardwareTime`, `GameWorld.DeltaTime`, `Player.UpdateTick`). `ManualUpdate` gets it and keeps it (`LastDeltaTime`), and the movement states get that kept figure. At 120 frames a second it runs 8, 8, 9 ms where the engine's is 8.33 each. The animator, and with it the root motion, advances by the engine's time, and so does any SmoothDamp called without a time (`_speed` below). What follows from the two clocks:

- The game's velocity figure is the frame's motion over the player's time, so at an even pace it reads high on the 8 ms frames and low on the 9 ms one: 3.12, 3.12, 2.78 m/s at a walk of 2.99.
- Smoothing done on the player's time (the direction, the speed parameter) runs a third of a millisecond ahead or behind and is level again every third frame.
- The hard cap on speed (below) is checked against that velocity figure. Motion near the cap is over it on the short frames and cut to the cap times 8 ms, and under it on the long frame and let through, so while the motion is within a few percent of the cap the character goes up to 3% slower than either the cap or the motion would have it.

The simulation takes both times (`Movement::step_on_clock`). `Movement::frame` takes one frame and keeps such a stopwatch itself. A host does not hand it the frames it draws: the game's rules work per frame, so that would make the result follow the host's frame rate, as the game's follows its own. It hands it frames of a fixed length from a clock kept for the purpose (`trp_sim::Pacer`, 120 a second: the rate of the recordings) and draws between their results. `Movement::step` gives the one time twice.

## Root motion table

The local player reads root motion from a live animator. The game also ships that motion precomputed, for characters that do not run one: `playerrootmotiontable.asset.bundle`. This project moved by the table at first and now runs the animator itself (`body-animator.md`). The table stays as a check: its velocities equal each clip's own average speed, and its stored weight grids are the engine's answers for the two-parameter blends, which the blend functions are tested against.

**Format.** A `RootMotionBlendTable` object lists the parameters (name, min, max, grid step) and points to a binary blob. The blob is a sequence of states until the end of the data, in .NET `BinaryWriter` layout (little-endian, strings prefixed by a 7-bit variable-length count):

```
state:  string name, int32 fullPathHash, node
node:   byte kind (1 clip, 2 tree), string name, bool looping, <kind data>, int32 childCount, node[childCount]
clip:   string name, float timeScale, float length, int32 clipIndex, float[3] velocity,
        bool storesRotation, bool usesCurves,
        if either: int16 curveCount, curve[curveCount]
curve:  int32 keyCount, int16 postWrap, int16 preWrap, key[keyCount]
key:    float time, value, inTangent, outTangent, inWeight, outWeight; int16 weightedMode
tree:   byte blendType (1 Simple1D, 2 FreeformDirectional2D, 3 FreeformCartesian2D, 4 SimpleDirectional2D, 5 Direct),
        int32 paramXHash, int32 paramYHash, int32 childCount,
        int32 n, float[n] thresholds,
        int32 n, float[2n] positions,
        int32 n, int32[n] directParamHashes,
        int32 stepsX, int32 stepsY, int32 gridChildren, bool storesRotation,
        float[stepsX * stepsY * gridChildren] weights      (x outermost, child innermost)
```

Hashes are CRC-32 of the name (Unity's `Animator.StringToHash`). The state key is the hash of the full path, such as `Base Layer.MOVE`; the table stores the bare state name beside it.

**Evaluation.** Velocity of a node, in the character's frame (x right, z forward), metres per second:

- Clip: its `velocity`, or zero if it does not loop and the state's normalized time has reached 1. Clips flagged `usesCurves` (vaults, kicks, climbs) take their motion from curves instead; not implemented.
- Tree: the weighted sum of its children, skipping weights below 0.01.
  - Simple1D: weights from the thresholds. A child has weight 1 at its own threshold, falling linearly to 0 at each neighbour's. Outside the ends the end child has weight 1.
  - Direct: each child's weight is the value of its own parameter.
  - The 2D types: weights are precomputed on a grid over both parameters. Each parameter's grid has `ceil(range / step) + 1` points starting at its minimum. Look up the four surrounding grid points and interpolate bilinearly.
- During an animator transition the game interpolates between the two states' results by the transition's progress. The result is rotated by the body's rotation.

**Contents that matter here** (185 states):

- `MOVE`: a tree over (`Direct_X`, `Direct_Y`) with one subtree per 45°, each blending six clips over (`Speed`, `Level`). Forward speeds in m/s: crouch slow 0.469, crouch 1.384, crouch run 2.052, walk slow 0.800, walk 2.502, run 4.032. Sideways and backward clips have the same speeds.
- `Sprint`: a tree over (`Direct_X`, `SprintSpeed`): 4.0, 5.0 and 5.5 m/s forward, with variants angled 20° left and right.
- `Idle_Aim` and the jump states: zero. Jump motion comes from code.
- `Land_Move`, `Transition_Sprint_to_Stand`, `Sprint_To_Prone`, sidesteps: short non-looping clips with their own velocities.

## Speed

- `CharacterMovementSpeed`: the player's chosen speed, set by the speed wheel. Clamped to `[0, MaxSpeed]`.
- `MaxSpeed` = `globals.config.WalkSpeed` evaluated at strength level / 60. The sprint equivalent uses `SprintSpeed`.
- `StateSpeedLimit`: the lowest of the active speed limits (weight, aiming, armour, swamp, ...), 1 when there are none.
- `ClampedSpeed` = `clamp(CharacterMovementSpeed, 0, StateSpeedLimit)`.
- `SmoothedCharacterMovementSpeed`: what the animator's `Speed` parameter receives. Produced by the move state's inertia, below.
- After a sprint ends, `CharacterMovementSpeed` is reset to `MaxSpeed`.
- The wheel's setting is also kept as a share of `MaxSpeed` (`_relativeSpeed`), updated whenever the speed is set through the wheel or that reset. At every change of movement state `CharacterMovementSpeed` is put back to that share times `MaxSpeed`. The wind-up before a sprint writes the speed directly, without touching the share, so the sprint's start undoes it: the recorded selected speed climbs to 1 and is back at the wheel's 0.686 on the sprint's first frame.
- **Aiming** adds a limit of 0.33 (plus a skill bonus, unread) for as long as it lasts.
- **A landing** adds a limit for a time (`OnJumpEnd`, on leaving the jump state). With `f = Inertia.SpeedInertiaAfterJump` evaluated at the inertia: the limit is `(1 - (BaseJumpPenalty + f^PenaltyPower - 1)) * MaxSpeed`, not below 0, for `BaseJumpPenaltyDuration + f^DurationPower - 1` seconds of real time. A second landing meanwhile keeps the lower limit and the later end. While it lasts the same figure also multiplies the sprint parameter's top. Here (0.3, 0.4 s, powers 1.23 and 1.07, `f` from 1 to 1.55): for the recorded character 0.456 for 0.43 s, which is what the recording shows after a standing jump.
- **A fall** adds to the same limit (`Player.LandingAdjustments`, called on the frame the character finds the ground after being off it). The height fallen is the highest point reached since it last stood on the ground, less its height when the ground check first holds again, which is read before that frame's move. Under `Inertia.FallThreshold` (0.15 m) nothing happens. Otherwise the limit is `SpeedLimitAfterFall(height)` for `SpeedLimitDuration(height)` seconds, each an eased curve between two keys from `Inertia`: here 1 at 0.3 m down to 0 at 4 m, and 0.5 s at 0.3 m up to 1.2 s at 2 m. It shares the landing's slot, where the lower limit and the later end are kept. A jump is a fall too, so a landing's cap lasts as long as the fall says when that is longer: the recorded running jump came down 1.11 m from its top and was capped at the landing's 0.456 for 0.82 s, not 0.43.
- **When a limit takes effect.** A changed limit is worked into the cap by the next once-a-frame look at the limits (`ProcessSpeedLimits`), which comes after the ground check and before the animator and the states. A fall's limit, set in the ground check, therefore applies in the same frame; a landing's, set when the animator leaves the jump, the frame after; and a limit whose time is up is dropped by one look and gone from the cap at the next. **The sprinting state looks again.** Its own update calls the same look just before it works out the sprint's pace, so in a sprint a limit set since the frame's look applies at once. A jump out of a sprint lands in the sprint (below), and the cap the jump leaves cuts the sprint's pace on that very frame: recorded, 1.833 to 0.827 on the first sprinting frame (eighth recording, 57), where the simulation without the second look had 1.683 for one frame.

## The hard cap on speed

Apart from all of the above, the character controller refuses to move faster than a cap that belongs to the movement state (`SpeedLimiter`, `SimpleCharacterController.CheckSpeedLimit`).

- The cap in metres per second runs from an untrained character to Strength 60 (whole levels, `level / 60`): standing and the other stationary states 0.5 to 1.5; walking, sidesteps and any state not listed 3.6 to 4.6; sprinting, the stop out of a sprint, jumping and falling 6.6 to 7.9; crawling 1.2 to 1.5. These are written in the game's code as its defaults; an online server may send others.
- It is multiplied by `lerp(Stamina.SprintSpeedLowerLimit, 1, WalkSpeedLimit)`, where `WalkSpeedLimit` falls from 1 with overweight; in a sprint the factor is 1.
- It is set at every change of movement state (to a different state).
- After the controller has moved, it works out the velocity from where it started and where it ended. Downward speed is not counted. If what is left is over the cap, the velocity is cut to the cap along its own direction (when falling, so that the level part is at the cap) and the position is put at start plus velocity times the frame.
- The motion the standing state remembers for its own blend (`LastBlendMotionDelta`) is taken before the cap, so it keeps running down behind it.

What this does: stopping from a walk does not coast. The standing state begins when the walk's blend-out starts, the cap drops to the standing figure at once, and the character moves at exactly that figure until the blended motion has fallen below it. The recorded character (Strength 40, cap 1.167 m/s) goes from 2.94 m/s to 1.17 in one frame, holds 1.17 for 0.125 s and then runs down. An untrained character is held to 0.5 m/s.

## Speed limits (`MovementContext._speedLimits`, `method_4`, `ChangeSpeedLimit`; `Player.UpdatePenalties`, `ApplyHitDebuff`; read 2026-10-25, built 0.133.0)

`StateSpeedLimit` is the lowest of the limits in a table kept by cause (`ESpeedLimit`), 1 with none; the sprint's own limit (`StateSprintSpeedLimit`) is the fall's alone. A limit set with a time (a landing's, a fall's, a shot's) ends at its time on the engine's real-time clock; the look at the limits each frame drops the one due soonest, if it is due. The causes, and where each stands:

| Cause | Limit | Where |
|---|---|---|
| `Weight` | the overweight's walk limit times the top | built (`MovementSettings::state_speed_limit`) |
| `Aiming` | 0.33 while the sights are up | built |
| `Fall` | a landing's and a fall's, for a time ("Speed", above) | built |
| `HealthCondition` | a damaged leg without a painkiller: 0.3 of the top, both 0.2 | built (`Movement::set_conditions`; `health.md`) |
| `BarbedWire` | 0.15 inside the wire | built (`Movement::set_in_barbed_wire`) |
| `Armor` | the armour's and backpack's penalty, below | built 0.133.0 |
| `Shot` | a weapon's hit, below | built 0.133.0 |
| `Swamp` | 0.2 in a swamp's water (`MovementContext`, its trigger) | **not built**: the range has no swamp; a host's world would tell it |
| `SurfaceNormal` | 1 from the start, never changed | nothing to build |

**Armour** (`Player.UpdatePenalties`, called whenever what is worn changes). Every armour component in the five armour slots (headwear, face cover, eyewear, rig, body armour) and in their own slots (plates, visors), destroyed ones too, adds its `speedPenaltyPercent` less its vest skill's share by its kind: a light piece's by 0.006 a level of Light Vests, a heavy one's by 0.005 a level of Heavy Vests (both held at level 50), none for a piece of neither kind; the backpack adds its own whole. Below nought, the speed is limited to `(100 + sum) / 100` times the top speed (`MaxSpeed`) as it stands then. (The same call sums the mouse's and the ergonomics' penalties: not this step's.) Built: `trp_sim::armor::speed_penalty` (the pieces gathered as a hit's are, `worn_pieces`), `Movement::set_armour_penalty`; the app works it out whenever what is carried or the skills change (the vest skills only while `SKILLS` is on), the replay from a recording's kit and skills.

**A hit** (`Player.ApplyHitDebuff`, from `ApplyDamageInfo`, every hit given to the player): with weapon damage (a bullet, a blow, a blunt hit; not a grenade's fragment or blast), the stamina burnt by the frame's hits together (each the round's `StaminaBurnPerDamage` times the damage, as the armour left it) gives `InverseLerp(55, 10, burnt)`: under 1, it is set as the shot's limit for 0.66 s (a limit already set keeps the lower figure and the later end). So under 10 nothing, 32.5 halves the speed, 55 or more stops the character for the while. And a leg's hit without a painkiller switches the sprint off (`Physical.Sprint(false)`). (The same call plays the hurt voice and burns stamina: the next steps.) Built: `Movement::hit`; the app hands it every hit given to the player (`contusion_view::Received`, which now carries the burn: nought for what no round did; `--damage <frame>:<part>:<amount>[:<burn a point>]` gives a capture run one).

**Compared with**: the armour's limit on 329 recordings (`limits` binary; `local\limits_compare_s135.txt`): the kit of each recording's first frame through our penalty and the recorded vest skills (40 and 40; `--vests 40,40` for the one that does not write them), beside the game's `state_speed_limit` on that frame: 327 to the sixth place, kits of 0, -2.4, -2.9, -3.0, -5.0 and -5.4 percent; the other two, `courses1\1-med-fracture`, held at 0.3 by the fractured leg, which is the health's limit, lower. The replay now slows a recording's character by its kit's armour (`after167`): in the M60E6's recordings (`r17`, `r18`, whose kit has armour) the selected speed's largest gap shrank in 10 scenarios (in `05-walk-aimed` from 0.021 to nought) and grew in none. A hit's limit and the leg's sprint: from the code only, `PROVISIONAL`: the one recording of the player shot (`courses1\0-death-shot`) is killed outright and writes no limit after; a recording of the player shot and living (a bot, or recorder 0.14.0's `+` by hand) would settle it.

## Idle (`IdlePlayerState`)

- `Move(direction)`: if any component exceeds 1e-5 in magnitude, the direction is reduced to its signs and `MovementDirection` is set to that times 0.01; sprint is enabled if sprint is held and the input has a forward component above 0.1; the `Inert` flag is raised, which is what takes the animator to `MOVE`. With no input, `MovementDirection` is zero.
- `BlendMotion`: the frame's motion is blended toward the previous frame's blended velocity by `EFTHardSettings.IdleStateMotionPreservation`, so the character does not stop dead.
- Entering idle turns sprint off.
- Jump: if `PoseLevel > 0.6` and grounded, try to jump; otherwise stand up fully instead.

## Moving (`MovePlayerState`)

State: `_speed` (2D, in world axes), its SmoothDamp velocity, `InertiaDirection` and its SmoothDamp velocity, `TransitionCoef`, `_lowerRange`, `_previousSpeed`, `_timeWithoutInput`, `moveTime`, `prevDirection`, `nextDirection`.

On entering: `_speed` starts at (0, 1) if coming from sprint, else zero. That (0, 1) is in world axes like the rest of `_speed`: it points along the world's z axis whichever way the player faces. Walking on from a sprint toward +z, `_speed` only has to shrink from 1 to the walking target; toward -z it has to cross zero, falls under `_lowerRange` on the way and is put at `_lowerRange` at once (step 9). So how the speed parameter comes down after a sprint depends on the compass bearing. `_lowerRange = max(0, ClampedSpeed - MovementAccelerationRange(WalkInertia))`. `InertiaDirection` starts as the current direction in world axes. The state's own input direction is set to the current `MovementDirection` too (coming from standing, the signs times 0.01) and keeps that value until the next input command, which arrives a frame later. So on the walk's first frame the direction smoothing aims at the point it already stands on, and only from the second frame at the key.

**What entering does not clear.** The game keeps one object per movement state for the life of the player, and entering the walking state resets most of its fields but not these: the two SmoothDamp rates (`_speed`'s and `InertiaDirection`'s), `TransitionCoef`, the last input seen, and a postponed sprint. A walk therefore begins with the rates the last walk's smoothing ended on. `_speed` has all but settled by then, so its rate is next to nothing. The direction's has not: the walking state is left about 0.1 s after the keys are let go, while the direction is still on its way down to zero, and its rate at that moment waits there, however long the player stands, until the next walk. That walk's direction is first carried the old walk's way backwards: after a walk forward, the forward direction of the next one dips below zero for four frames before it rises, and a strafe begun then has a backward part of up to 0.12 for half a second. The sprint state is a second object of the same class with fields of its own, so a sprint does not disturb what the walking state holds.

Each frame (`UpdateMovementDirection`), with `Direction` the raw input:

1. **Reversal pause.** If the input reverses (dot of the previous and new normalized directions below 0) and no pause is running, start one of `Inertia.MoveTimeRange(Inertia)` seconds. While it runs the input is treated as zero; when it ends the new direction takes over and `Inert` is cleared for a frame.
2. Convert the input to world axes using the body's transform.
3. `target = clamp(ClampedSpeed, 0.01, 1)`.
4. `crouchFactor = lerp(1, Inertia.CrouchSpeedAccelerationRange(ClampedSpeed), CrouchAccelerationWeightRange(1 - PoseLevel))`.
5. `desired = normalize(input) * target * speedAcceleration * crouchFactor`. `speedAcceleration` is 1 standing, a prone-specific value when prone.
6. `TransitionCoef`:
   - input just became zero: `SmoothedCharacterMovementSpeed`;
   - input just became non-zero: `target`;
   - `target` changed since last frame: `|target - |_speed||`, and `_lowerRange` is recomputed;
   - otherwise unchanged.
7. `smoothTime = TransitionCoef * WalkInertia * directionFactor`, where `directionFactor` is `MoveDiagonalInertia` for diagonal input, `MoveSideInertia` for pure sideways, 1 otherwise. If decelerating (`target < SmoothedCharacterMovementSpeed`, or no input), divide by `EFTHardSettings.DecelerationSpeed`.
8. `_speed = SmoothDamp(_speed, desired, velocity, smoothTime, maxSpeed = InertiaInputMaxSpeed)`.
9. If `|_speed| < _lowerRange`: `_speed = desired / target * _lowerRange`.
10. Unless sprinting: `SmoothedCharacterMovementSpeed` follows `|_speed|`. If they are within 0.001 of each other (`LowAccuracyApprox`) it is set directly, otherwise it moves toward it by `lerp(current, |_speed|, Inertia.SuddenChangesSmoothness * dt)`.
11. **Direction.** `InertiaDirection = SmoothDamp(InertiaDirection, input, velocity2, max(Inertia.MinDirectionBlendTime, smoothTime) / directionSpeedAcceleration, InertiaInputMaxSpeed, dt)`. `MovementDirection` is that converted back to the body's axes.
12. Time without input accumulates while the input is zero.

Leaving the state: when there has been no input for longer than `Inertia.MaxTimeWithoutInput(Inertia)`, `|_speed|` is below `Inertia.ExitMovementStateSpeedThreshold(Inertia)` and no reversal pause is running, `Inert` is cleared and the animator returns to idle.

**The floor's curve is shared.** `_lowerRange` (entering, and step 6) is the selected speed less `MovementAccelerationRange` at the walk's inertia, and that curve is one object on the game's settings singleton, for every character in the raid. Whenever any character's carried weight changes (`PhysicalBase.OnWeightUpdated`: picking up, dropping, a magazine out, every round fired), that character rewrites the curve's second key with its own inertia: time and value interpolated between `MinMovementAccelerationRangeRight` and `MaxMovementAccelerationRangeRight` (here time 0.8 down to 0.3, value 1). A bot does it like the player. So the floor under the player's walk is worked out with the inertia of whichever character's weight changed last, at the moment the walk begins. Alone, or just after a shot or a reload of its own, that is the player's; otherwise it is some bot's. Nothing else reads the curve.

What it does: the speed parameter of a walk starts on the floor, and a crouched walk never leaves it (its target, cut by the crouch factor of step 4, lies below the floor), so a crouched walk's pace is the floor's. For the recorded character (walk inertia 0.0273, own inertia 0.068) the floor is 0.5979 by its own key and 0.5877 by a fully laden character's, 1.7% apart; an upright walk only starts from it and ends within a millimetre either way. A host with one character gets its own key, which is what the simulation does unless told (`MovementSettings::key_acceleration_range`). Found in the tenth recording and confirmed in all of them: see "Compared with the running game".

Body rotation while moving: a smoothed yaw follows the look yaw by `LerpAngle(smoothed, yaw, TRANSFORM_ROTATION_LERP_SPEED * dt * k)` with `k = 1 + inverseLerp(10, 45, |angle difference|)`.

Sprint from this state needs forward input above 0.1. It sets the stance to 1; if that fails (no headroom) sprint is refused. The animator's `Sprint` flag is raised once `PoseLevel > 0.9` and `SmoothedCharacterMovementSpeed >= 1`. Until then `PreSprintAcceleration` raises both speeds by `Physical.PreSprintAcceleration` per second.

## Sprinting (`SprintPlayerState`)

- Entering: stance to 1, lean to 0.
- Each frame while forward input is held and sprint is enabled: `MovementDirection = lerp(MovementDirection, input, dt * DIRECTION_LERP_SPEED)`, then `SprintAcceleration`:
  - `step = Physical.SprintAcceleration * dt`
  - `limit = (Physical.SprintSpeed * SprintingSpeed + 1) * StateSprintSpeedLimit`
  - `turn = max(sprintSpeedInertiaCurve(|average yaw rate|), sprintSpeedInertiaCurve(huge) * (2 - Inertia))`
  - `limit = clamp(limit * turn, 0.1, limit)`
  - `SprintSpeed = clamp(SprintSpeed + step * sign(limit - SprintSpeed), 0.01, limit)`
  
  `SprintSpeed` is the animator parameter of the same name.
- **Where `SprintSpeed` starts.** A character is made with it at 1. Entering the sprint state starts a watch on the next change of movement state, and when that change comes and is to anything but a jump, `SprintSpeed` is put to `StartingSprintSpeed` (0.5 in the settings object). So a character's first sprint winds up from 1 and every later one from 0.5; and a jump out of a sprint keeps the value, so landing back into the sprint goes on at the pace it left with, and if that jump ends in a walk the value is kept until a later sprint ends. Recorded (seventh recording): the parameter stands at 1.000 through the first five scenarios, the first sprint (06) winds up from 1 and reaches its 1.833 in 0.90 s where the sixth recording's, from 0.5, took 1.43 s, and it is 0.5 from that sprint's end on. The jump's exception is recorded too (eighth recording, 57: a jump out of a full sprint with the keys held). The movement state goes sprint, jump, sprint; the parameter stands at 1.833 through the flight, is cut to 0.827 on the first sprinting frame by the cap the jump leaves ("When a limit takes effect") and climbs back as that cap runs out; it is never put to 0.5. The simulation has the parameter on every frame and ends the 20.8 m within 0.6 mm.
- Sprint ends when forward input is released, sprint is disabled or cannot continue. Crouching while sprinting starts a slide ("The slide out of a sprint", under "Slopes and sliding").
- Leaving: sprint off, speed reset to `MaxSpeed`.
- Lean and the speed wheel do nothing while sprinting.

## A frame with no input (read 2026-10-11, evening)

The movement keys are not events. Every frame the player's input node calls `Player.Move` with the direction the keys give, even when that is none, and each movement state does with it what it does. While a screen is open (the inventory, a menu) the node is handed no axes and `Move` is **not called at all** (`inventory.md`, "What an open screen does to the keys"). What each state makes of such a frame:

- **Walking and crawling** (`MovePlayerState`, and the crawl built on it). `Move` only stores the direction. The update that follows ends by counting up a time "without a call to move" and, once that is over `MAX_WITHOUT_MOVE_CALL` (0.2 s), putting the stored direction to nothing. The flag that should set the time back when `Move` is called is never set: `Move` clears it, where it was plainly meant to raise it (checked in the game's own bytecode, not only in the decompiler's reading), and nothing else writes it. The time belongs to the state's object and is cleared by nothing, entering the state included. So from a fifth of a second into a character's first walk, **every update of the walking state ends by dropping its input**, and the next frame's has to be handed over anew. A frame in which it is not is a frame with none: the walk runs down exactly as when the keys are let go. The same goes for a second update of the state in one frame ("A state's update can run twice in a frame", under "Looking"): it runs with none. One exception: the update in which a reversal's pause ends puts the new direction in and returns before the dropping, so that direction stands for one more update.
- **Sprinting.** Its update lerps the movement direction toward the stored one and then puts the stored one to nothing itself, noting the frame it did so in. A later frame that finds no forward ends the sprint (the same test as for the key let go); a second update within the noted frame does not, and sprints on with no direction.
- **A jump** keeps the direction `Move` last gave it, so its steering in the air goes on as it was.
- **Standing** and the states built on it: nothing is asked. The movement direction and the animator's `Inert` flag stay as the last call left them.
- **The view** is not turned: `Look` and `Rotate` are not called, so nothing is added to the turns of the last frames that a sprint's pace goes by either.
- `Player.InputDirection`, which a try at a vault goes by, keeps what `Move` last gave it.

**Built** (`Movement::unheard`, and the dropping in `update_movement_direction` and `update_sprint`): a host says a screen has the keys with `Input::screen`. The simulation starts the walking state's time past its limit, as a character met any later than its first fifth of a second of walking has it. **Checked:** a walk and a sprint with the keys still down under `screen` are on every frame where one with the keys let go is (`a_screen_takes_the_keys_from_a_walk`, `a_screen_ends_a_sprint`: 0.474 m and 0.855 m to rest), and the fourteen recordings replay byte for byte as before with the dropping in (`after23` against `after22`), so no frame in them shows a second update of the walk or the sprint to any effect. From the code alone: no recording has a frame without input.

## Jumping (`JumpPlayerState`)

**The key.** In this install the jump is bound to the release of Space, because the same key held is the vault key (`vaulting.md`, "Two moves, one key"). `Player.Jump` refuses the jump when the vault key has been held longer than `VaultingInputTime` (0.6 s) or a vault is under way. A recorder's script presses the jump as a command of its own, so the recordings' jumps come at the script's time. In the simulation `Input::jump` is the command and `Input::vault` the held key; a host with one key for both gives the jump when it is let go.

Not root motion. On entering:

- `v0` = the horizontal velocity before the jump: per axis, whichever is larger in magnitude of the last pre-limit input velocity and (if coming from the move state) the normalized movement direction.
- `initialMagnitude = |v0|`; direction is `v0` normalized, or the body's forward if the magnitude is at most 0.1.
- `lift = LIFT_VELOCITY_BY_SPEED(initialMagnitude) * weightFactor`, upward, where `weightFactor = lerp(1, 0.66, Overweight)` (0.66 with a damaged leg and no painkillers). Strength adds a multiplier. (The weight factor was left out of the simulation until 0.10.0, when health brought the damaged leg's: no recording was overweight, and the regression did not move.)
- `liftDelay = JUMP_DELAY_BY_SPEED(initialMagnitude)`.

Each frame, with `t` the time in the state:

- vertical velocity: zero before `liftDelay`, then `lift + gravity * (t - liftDelay)`;
- horizontal speed moves toward a target by `lerp(current, target, dt * AIR_LERP)`, where the target depends on the input relative to the jump direction: same direction (dot > 0.966) `max(AIR_MIN_SPEED, initialMagnitude * AIR_CONTROL_SAME_DIR)`; opposite (dot < -0.966) `initialMagnitude * AIR_CONTROL_BACK_DIR`; otherwise `initialMagnitude * AIR_CONTROL_NONE_OR_ORT_DIR`;
- motion = `(direction * speed + vertical) * dt`. The direction itself never changes in the air.
- **That `dt` is the player's whole-millisecond clock** ("The player's clock"), so on frames of even length the travel through the air is uneven: 8, 8 and 9 ms of it a frame at 120 frames a second, a change of an eighth from one frame to the next, where travel on the ground is the animator's and even. Measured in the simulation on a running jump: 25.0, 25.0 and 28.1 mm a frame in the air against 23.7 every frame before it. The recorded jumps agree with the simulation's positions to the millimetre frame by frame, so the game moves so too. It shows as a judder of the world in a jump. **The simulation keeps it and gives a host a second figure to draw** (`Movement::drawn_position`, built 2026-10-08 when the user reported the air as jittery): the level place the same jump has reached by the frames' own time, which is `position + velocity * (how far the player's clock has fallen behind the frames since the jump began)`, never more than a millisecond's travel away. Its travel changes by under 1% from frame to frame. The height is left alone: the eye's already trails the feet's through a spring. This is ours, not the game's; the comparison with recordings goes on using `position`.
- **The body turns after the view all through a jump**, with no limit to wait for: each frame `body = Quaternion.Lerp(body, the view's yaw, TRANSFORM_ROTATION_LERP_SPEED * dt)`. That is the engine's blend of two rotations: their quaternions are mixed component by component and the result made a unit again, which for two turns about the same axis `a` degrees apart gives a turn of `2 * atan2(t * sin(a / 2), 1 - t + t * cos(a / 2))`, not `t * a`. The two differ by the cube of the angle: 0.3% of it at 25 degrees. Recorded (seventh recording, 45: the view turned 28 and 30 degrees in a jump): with the angle's share the body was up to 0.041 degrees off through the jump; with the engine's blend it is the game's to four decimal places on every frame. The standing states' follow (`ProcessUpperbodyRotation`, under Looking) is the same blend at `30 * dt`.
- The jump ends when it has been descending for longer than `JumpTimeDescendingForStateExit` and is grounded, or when the head hits something, or when height stops changing for more than 3 frames.
- On landing with input held, movement resumes, and sprint resumes if the jump started from a sprint and forward is still held.
- The landing state (`JumpLandingState`) clears its input on entering and is handed the keys again only by the next frame's input. Its first frame therefore counts as one without input, and takes one step of the speed parameter toward 0.35 (`lerp(speed, 0.35, SuddenChangesSmoothness * dt)`): recorded, 0.686 to 0.677 on that frame and no further.

## Gravity (`MovementContext.ApplyGravity`)

For states that stick to the ground (all but the jump):

- In free fall for 0.25 s or more and not grounded: `motion *= 1 / (1 + 2 * FreefallTime)`, then `motion += G * FreefallTime * dt`.
- Else on a slope that should slide: `motion += Slope * G.y * dt`.
- Else: `motion += G * FreefallTime * dt`.

`FreefallTime` is zero while grounded and accumulates otherwise.

**What counts as on the ground** (`UpdateGroundCollision`, first thing each frame). The character is on the ground if its controller touched ground in its last move, or if a ray straight down from the capsule's centre meets ground within half the capsule's height, the skin width and a check distance. The centre sits half a height and the skin above the feet, so standing the feet may be as much as the check distance over the ground. The check distance is 0.07 m to begin with and the player sets it at a change of movement state: 0.08 on entering the walk or the crawl, 0.15 on entering the sprint, 0.03 on entering the jump. Any other state leaves it as the last of those set it.

Gravity only acts off the ground, so nothing pulls a character down that is within the check distance. Where the ground falls away under a walk the character keeps its height until the gap passes 8 cm, drops for a frame or two until it is inside again, and so rides up to 8 cm over the ground; sprinting, up to 15 cm. It comes down onto the ground itself only out of a jump or a real fall. Recorded on ground sloping about 0.4%: a walk loses 2 mm in two frames every 0.55 m, a sprint covers 15 m without losing any, and a standing jump lands 8.0 cm below where the character had been standing, on the same spot.

## Falling (read on 2026-10-09; first recorded on 2026-10-10, below)

**There is no falling state.** The code has a class for one (`FallDown`, the jump's state with no push-off) and the list of movement states has its number (7), but the place where the state objects are made leaves that number empty, and nothing asks for it. The body's animator has a looping `Fall` state and a `Fall_End` after it, and no transition leads into either: the controller has no transition from any state at all, and `Fall` is only where its jump group would start if something entered the group as a whole, which nothing does. So a character that walks off an edge stays in the state it was in (walking, sprinting, standing, lying) and its legs go on as they were; what makes it a fall is gravity and the ground check alone.

- **Going over.** Each frame begins with the ground check (above, "What counts as on the ground"). Off the ground, `FreefallTime` counts up and the frame's motion gets `G * FreefallTime * dt` added: a speed of `g * t` downward. The motion itself is still the animator's, so for the first quarter second the character keeps the pace it walked or ran off with.
- **From a quarter second in** the whole motion is first multiplied by `1 / (1 + 2 * FreefallTime)` every frame: two thirds at 0.25 s, half at 0.5 s, a third at 1 s. The walk forward dies away in the air; a fall from a height comes down nearly straight.
- **Three things the code tells the animator and the animator does nothing with:** `IsGrounded`, `FallingDownFloat` (how far the ground is beyond the check distance, as a share of 0.3 m; 1 if no ground is found) and the flag `FallingDown`, which the walking state raises when it finds itself off the ground. No transition and no blend tree of the player's controller reads the last two; `IsGrounded` is read only by the landing out of a vault.
- **The sights.** The firearm's own update, which comes after the movement's in a frame, puts the sights down on every frame on which `FreefallTime` is over 0.5 s, through the same call as the aim key (so a reload in hand below mastering 2 and a look at the weapon ignore it). They stay down after the landing until the key is used. Half a second is a drop of 1.2 m.
- **Landing** (`CheckFlying`, in the frame's update after the ground check). The height fallen is measured from the highest point since the character last stood on ground. On the frame the ground is found again: a fall of `Inertia.FallThreshold` or more caps the selected speed for a while (under Speed, above; built and recorded for jumps), and the health system is told the height (`HandleFall`), which is where the harm from a fall comes from. On the ground `FreefallTime` goes back to nothing.
- A jump is a state of its own with its own arc; it keeps `FreefallTime` running, so a long jump down from something puts the sights down in the same way (they are down in a jump anyway).

**Built** (2026-10-09). The ground check, the gravity, the slowing and the landing's cap were in already for the walk over falling ground and for jumps. New: `Movement::long_fall` says when the fall has lasted long enough to put the sights down, and hosts lower them with it each frame. The animator's three parameters are left unset, since nothing reads them. **Not built:** the harm (there is no health yet), the landing's sound, and what the game's character controller does at the edge itself: its capsule has a rounded foot 0.37 m across, and whether the character is still "on the ground" with its axis past the edge and the foot's side resting on the corner is the engine's collision, which is not rebuilt. Here the character is over the edge when its axis is.

**Recorded, 2026-10-10** (thirteenth recording's vault traces: out of each climb the character walked across the wall's top and off its far side, 1.12 m down, at a walk). What the edge does is settled: on the top the capsule touches and does not overlap, so the character is on the ground only by the ray under its axis; on the frame after that ray loses the top, gravity begins (`FreefallTime` 0.009, 0.017, 0.025 on the game's own clock), and at a walk the capsule is clear of the corner before it has fallen onto it. The pace over the ground holds for a quarter of a second and then dies away as read: 3.12 m/s for 0.25 s, then 1.85, 1.73, 1.63, 1.52, 1.43 by the game's figure every twentieth of a second, against 1.83, 1.72, 1.62, 1.51, 1.42 here. The fall takes 0.485 s, short of the half second that puts the sights down. The simulation's fall runs one frame ahead of the game's, because the wall the replay rebuilds from the trace stands 3 cm further on (`vaulting.md`, "Compared with the running game"). A fall from a walk off open ground higher up, the sights going down in it, and the harm are still not on record.

**Measured in the simulation** (`cargo test -p trp-sim --test sim a_walk_off_a_ledge -- --nocapture`; a walk at full pace straight off a ledge 2.5 m high). The figures are under "Compared with the running game", at the end. The recorder has a scenario for it (`hosts\spt-oracle\scenarios-ledge\50-fall-from-ledge.json`, run alone, standing 3 m back from a ledge and facing it), and from 0.4.3 it writes how far under the feet the ground is on every frame (`ground_below`), which is the ledge's own profile.

## Stance (`PoseLevel`)

- `PoseLevel` is 0 (crouched) to 1 (standing), capped by `Physical.MaxPoseLevel`. Changes are refused if there is no headroom.
- Capsule height: `lerp(1.2, 1.6, PoseLevel)`, 0.3 prone. Step offset 0.25 (0.15 prone), slope limit 60° (45° prone).
- `SmoothedPoseLevel` follows it: `lerp(smoothed, target, dt * POSE_CHANGING_SPEED * rate)`, where `rate` is `Physical.PoseLevelIncreaseSpeed` going up and `PoseLevelDecreaseSpeed` going down; it snaps when within 0.001. Standing up costs stamina.
- A temporary downward offset (`PoseInertia`, decaying by the `PoseInertiaDamp` curve) is applied after landings.

**Which states take a change of stance** (read again on 2026-10-10, after the thirteenth recording showed the crouch key taken in mid-jump). A change goes to the movement state in hand (`ChangePose`), and what it does is a matter of which class that state's object is:

- The plain handler (`MovementState.ChangePose`) sets the stance and nothing else. It is what every state has that does not replace it: standing, **the jump**, the landing, the sprint's stop, going down, the sidestep, and the two states that follow a vault.
- Walking (`MovePlayerState`) sets it and, with the stance then under 0.9, switches the sprint off.
- Sprinting takes only a change downward: straight to a full crouch, and out of the sprint (the slide: "Slopes and sliding").
- Lying still and crawling have their own (under Prone).
- In a vault or a climb nothing changes: `SetPoseLevel` itself refuses in those two states, whoever asks.

So a stance change made in the air is taken: the target changes at once and the smoothed stance follows it through the jump at its usual rate. **Recorded** (thirteenth recording, `61-legs-dry`: the crouch key 0.3 s into a standing jump and again 0.8 s later, a tenth of a second after landing with the jump's state still in hand): the target goes to 0 on the key's frame and back to 1 on the second, and the smoothed stance is 0.861 a twentieth of a second after the first and 0.13 at the second. The simulation had refused both, from a reading that lumped the jump with the vault.

**The crouch key** (`Player.TogglePose`) asks for the stance last *chosen with the stance keys* (`PoseMemo`) if the stance is not there now, and otherwise for a full crouch from half-way up or more and for standing from below that. It does not itself write `PoseMemo`: only the stance keys do (`Player.ChangePose`), and the making of the character (standing). So from a stance of 0.7 set with the wheel the key goes 0, 0.7, 0, 0.7, not 0, 1, 0. The simulation had the key write the memo too, which is the same thing from standing and from a full crouch, the only cases any recording has; the rule as read is from the code alone.

**Approaching something to open it** changes the stance too (the crouch to a low container, and back): `inventory.md`, "Opening a container"; `Movement::approach` and `let_go`, from the code, not recorded.

## Prone

Four movement states, each named by animator states of the base layer (`body-animator.md`): going down (`Transit2Prone`), lying still (`ProneIdle`), crawling (`ProneMove`) and getting up (`Prone2Stand`).

**The command** (`Player.ToggleProne`, then the current state's `Prone`).

- Lying still or crawling: `IsInPronePose` is cleared and the stance is set back to `PoseMemo`, the stance the player last chose with the stance wheel or the crouch key.
- In every other state: `IsInPronePose` is set, if the character may lie down (`CanProne`: conditions on health and medicine, and room to lie in: "Room to move"). Nothing else happens in code: the flag is the animator's `Prone`, and the animator leaves standing, walking or (moving forward) sprinting for a clip that goes down.
- Standing up needs headroom for the stance wanted, or for a crouch failing that (`CanStandAt`: "Room to move"), and takes stamina.
- Jump, lying down, is the same as the command. The stance wheel and the crouch key, lying still, get the character up into a full crouch (stance 0); crawling, they get it up into the stance held before.
- **In a reload the command is taken, by the player this was recorded from.** The game has two movement contexts. The plain one (`MovementContext`), which an offline raid's player gets, sets and clears `IsInPronePose` whatever the weapon is doing. The one an online raid's own player gets (`ClientPlayerMovementContext`) overrides the setter and drops the change while the body animator's `ProneReload` flag is up, which the weapon's logic keeps up from the start of a reload until the new magazine is in (`firing.md`, "Reloading with a magazine"): there the key does not take a standing character down and does not get a lying one up, nor do jump and the stance keys lying down, though the lying command still sets the stance back to `PoseMemo`. Every recording is of the offline player. Recorded (seventh recording, 46): the key pressed half a second into a reload, standing, takes the character down at once, on the frame of the key, with the reload going on; twice. What the key does lying down in a reload is then the animator's: `Prone` drops at once, the reload's state has no way out until `ProneReload` drops, and the body gets up when it does. Recorded (eighth recording, 56): pressed 0.8 s into a reload lying down, the key drops the prone flag on its own frame, the body lies on for 1.71 s until the reload's operation hands over, and the getting-up state begins two frames after that; the simulation has the flag, `ProneReload`, the movement state and the animator's state on every frame. The simulation follows the offline player; `MovementSettings::prone_key_refused_in_reload` switches the online player's refusal on, which is read from the code and compared with nothing.

**Going down** (`Transit2ProneState`, a standing-type state).

- On entering: the transition clips' speed is set to `Physical.TransitionSpeed` (the animator's `TransitionMultiplier`); the body's turn limit behind the view (`TrunkRotationLimit`) becomes 25 degrees; the view's pitch range starts closing toward the prone range; the capsule becomes the prone one.
- Each frame: the body turns after the view if past the limit, as standing; the animator's motion is taken as it is (no carry-over of the last motion as standing has), gravity is added, and the character is moved. If there is no ground or no room ahead or behind, the motion along the body is replaced by a push back out ("Room to move").
- Movement keys do what they do standing: they set the direction to a hundredth and raise `Inert`, which decides whether the clip ends in the crawl or in lying still. Jump does nothing.

**Lying still** (`PronePlayerState`, a standing-type state).

- On entering: the stance is forced to 0 (the smoothed one too) and the pitch range set closing toward the prone range.
- Each frame: the animator's motion, gravity, the move; the body turns after the view when more than `TrunkRotationLimit` from it, by `30 * dt` of the excess; and the view's yaw is then limited to 35 degrees either side of the body. With the limit at 25 since going down, the view leads the body by 25 to 35 degrees in a turn.
- A turn is refused altogether when the body would swing over an edge (`IsAbleToRotateProne`: "Room to move"; its other half, into something, is the head's: "The head's push out of walls").
- Movement keys as standing; lean as standing.

**Crawling** (`ProneMovePlayerState`, the walking state with these differences).

- The two factors that are 1 on foot are read: `desired` is multiplied by `Inertia.ProneSpeedAccelerationRange(Inertia)` (1 to 1.6), and the direction's smoothing time divided by `Inertia.ProneDirectionAccelerationRange(Inertia)` (2.5 to 2).
- The stance is forced to 0; the yaw limit is lifted; lean is held at 0.
- The sprint key does not sprint: while it is held the speed setting goes up a wheel notch a frame until it reaches the top.
- The crawl's blend takes the direction only, not the speed parameter, so the wheel does not change the pace. The hard cap for this state is the crawling one (1.2 to 1.5 m/s).
- Motion toward ground that is not there is turned back (`LimitProneMotion`: "Room to move").

**Getting up** (`Prone2StandState`, a standing-type state).

- On entering: the capsule is the upright one again, the transition clips' speed is set as for going down.
- Each frame: as standing.
- On leaving: the yaw limit is lifted, the pitch range is the standing one at once, and `TrunkRotationLimit` is the standing figure again. Until then the yaw limit is whatever lying still last set. Jump does nothing.

**The view's range.** Pitch is kept between two limits, and the limits move toward their targets by `lerp(limit, target, 5 * dt)` each frame, taking the target once within 0.1 of it. Upright the range is -90 to 90; lying down it is 16 degrees up to 25 down.

**Other figures.** The capsule lying down is 0.3 m high. `Physical.TransitionSpeed` is `Stamina.TransitionSpeed` (1 to 0.66) evaluated at the overweight, or at 1 when exhausted. The hard cap going down and getting up is the general one (3.6 to 4.6 m/s), lying still the standing one.

**Lying to the ground** (`MovementContext.AlignToSurface`, `RestoreDefaultAlignment`). The body's animated part is a child of the player and has a rotation of its own. Upright it is none. Going down, lying still and crawling, the game sets it at the end of every frame's state update:

1. Two points of the ground: the one under the capsule's centre (the ground check's own ray) and one found by a ray straight down from 1.5 capsule heights up, half a metre ahead along the player's forward, reaching 2 m. If both hit, the wanted direction is the unit vector from the first to the second. If only one hits, it is the player's forward laid into the plane of that hit's normal.
2. `direction = lerp(the animated part's forward now, wanted, 5 * dt)`. The rate is `PRONE_ALIGN_SPEED`, 5 in the settings object. It starts from the forward as it stands, not from last frame's direction.
3. The ground's normal (the ground check's, or the capsule's own contact normal where that ray finds a steep face) is limited to 30 degrees from upright, and `normal = lerp(normal, that, 5 * dt)`; it starts upright.
4. The animated part is turned to look along the direction with the normal as its up.
5. Its rotation relative to the player is then read as Euler angles and the roll limited to between -8 and 8 degrees. **The engine reports those angles from 0 to 360**, so a roll the other way reads as nearly 360 and the limit turns it into 8: a body lying across ground that is lower on its right by more than 0.0057 degrees (the engine's own allowance below zero, 0.0001 rad) is rolled a full 8 degrees to the left, and stays so. Ground lower on the left gives the true roll, up to 8.
6. `SurfacePitch` is the pitch of that rotation. The animator's `Aim_angle` is the view's pitch less it, and the view's pitch limits are shifted by it.

What hangs on it: the animator sits on the animated part, so its root motion comes out turned with it (a crawl to the right with the body rolled 8 degrees left climbs 14 cm a metre, until the character is further off the ground than the ground check reaches and gravity takes over), and every bone, the ribcage with them, is turned about the player's feet. The first-person rig hangs at the ribcage but keeps the view's rotation.

Getting up, nothing changes until the clip is 0.3 through. From there, with `t = (time - 0.3) / 0.7`: `direction = lerp(direction, the player's forward, t)`, `normal = lerp(normal, upright, t)`, and the animated part looks along them, with no limit on the roll. The normal here is the true one, so the false 8 degrees is gone in that one frame. On leaving the state the rotation is cleared.

Recorded (fifth recording): on ground rising 0.21 degrees ahead and to the left of the way the player faced, the roll appears in one frame, the sixth of going down, which is when the eased normal first passes the allowance; the eye is then 20 cm to the left of where it would be. Facing the other way on the same ground (fourth recording) there is no roll.

**Reloading lying down.** The body's animator answers the reload's flag with a state of its own that names lying still and has no way out until the flag drops (`body-animator.md`, "Reloading"). A reload begun in a crawl therefore ends the crawl: the movement state becomes lying still as the blend into that state starts, and lying still hands the animator a direction a hundredth the size, so the crawl's clip gives a twentieth of its travel from the next frame on, fading to nothing over the blend's 0.32 s. Until the new magazine is in, the movement keys and the step key do nothing: lying still raises `Inert` and sets `Step` as always, and the animator does not look at them.

Compared with the game (sixth recording, 39: a reload lying still): the animator's state, the state it is blending into and its clock are the game's on every frame, in at frame 360 over seven frames and out at frame 660 with no blend, the frame after the reload's flag drops. Seventh recording: a reload begun in a crawl with the key held (47) ends the crawl on the frame of the reload and the crawl starts again, the key still held, on the frame the reload's flag drops, 2.5 s later; the movement state and the animator's state are the game's on every frame and the distance crept within 1.9 mm. A character taken down by the prone key in a reload (46) lands in lying still and goes on into the reload's state from there, as the game's does, on every frame.

The push back from an edge, the refusal to turn over an edge and the roll's refusal where there is no room or no ground to that side are built since 0.131.0 ("Room to move"); the refusal to turn into something is the head's push out of walls.

**The roll** (`RollPlayerState`). The step key lying still rolls the body onto that side.

- *The key lying still.* It does not go to the animator at once. If a roll to that side is possible (a ground check and a box to that side), the lean is set to 0 and the step is remembered, unless the lean already pointed that way, in which case `Step` is set at once. At the end of the lying state's update, once the smoothed lean is under 0.1, the remembered step is given to `Step`. The animator has already run by then, so it starts the roll a frame after the key. Taking the key back sets `Step` to 0 but does not forget a step still waiting.
- *The animator.* Six states shaped like the sidestep's, all naming the movement state `Roll` (number 22): a way in (0.767 s, played at `SidebackSpeed`, entered from lying still with no blend when `Step` asks, or over half the lying clip if the lean was that way), a hold, and a way out (0.733 or 0.767 s) that ends in lying still, or in the crawl if a movement key is down. A second layer (`Additive_Aiming`) changes to clips for the rolled body while `Step` asks.
- *Entered:* the view's yaw is held to the trunk's limit less one degree (24 lying down) either side of where it points; the pitch range moves to 16 up and 2 down at the usual rate; the body's yaw as it stands is taken as the start of an easing; `Inert` is lowered; the playback speed goes to where it rests, as in the sidestep.
- *Each frame:* the animator's motion with gravity and the hard cap, with none of standing's carry-over; the eased yaw moves toward the bearing the view had on entering by `lerpAngle(yaw, bearing, 5 dt)`, and the body is set to that yaw with whatever the clip turned the root this frame on top; then the playback speed. The body is not laid to the ground while it rolls and does not follow the view.
- *Left:* the yaw is free and the pitch range is lying still's at once; `Step` is set to 0.
- The lean keys do nothing in it; jump only takes the step back; a change of stance or the prone key gets up as lying still does. Its hard cap on speed is standing's. Aiming is allowed.
- Its clips carry no travel. The way in and the hold each carry a first-person pose (`weapon-handling.md`, "First-person poses of body states") that turns the weapon over and rolls the camera.

Compared with the game (sixth recording, 38: right, back, left, back): `Step` is set on frame 359 and the `Roll` state begins on 360 in both; the movement state, the animator's state and how far through it are the game's on every frame; the eye, which swings 44 cm aside and 10 cm down while the player's place does not change, is the game's to under 0.1 mm; the weapon is within 4.4 mm and 0.65 degrees (root mean square), 8 mm and 2.7 degrees at worst, and the camera's turning within 0.2 degrees. Before it was built the same comparison gave 650 mm at the eye and 70 degrees at the weapon.

Compared with the game in four recorded scenarios (lying down and getting up, crawling three ways, a dive out of a sprint, turning the view while lying): every change of state falls on the game's frame. Figures under "Compared with the running game", the fourth recording.

## The sidestep (`SideStepPlayerState`)

A held key steps the standing character half a pace out to one side and leans it there; letting go brings it back. Read from the game's code and the sixth recording (scenarios 36 and 37).

**The key.** Pressing it calls `ToggleStep(side)`, with the side 1 for right and -1 for left: if the step now asked for (`Step`) is not that side, the state in hand is asked to set it; if it already is, it is taken back. Letting go calls `ReturnFromStep(side)`: if `Step` is still that side, the state in hand is asked to set it to 0. Only the standing-type states listen: standing still and lying still set `Step` as asked, the walk, the sprint, the crawl and a jump ignore the key. Standing refuses a step that would go into something or over an edge (a box and two ground rays to that side: "Room to move").

**The animator does the stepping.** `Step` is a whole-number parameter of the body's animator. From standing still it goes, with no blend, into `SideStep_In_Right` when `Step` is above 0 and `SideStep_In_Left` when below. Six states, all naming the movement state `Sidestep`:

| State | Clip (by stance) | Left by |
|---|---|---|
| `SideStep_In_Right`, `..._Left` | the step out, 0.333 s, played at `SidebackSpeed` times its speed | at its end to the hold; if a movement key is down, over 0.3 of itself to the way out |
| `SideStep_Right`, `..._Left` | the hold, a pose | to the way out, with no blend, when `Step` no longer asks for that side or a movement key is down; also for going prone, a kick, an interaction |
| `SideStep_Out_Right`, `..._Left` | the step back, 0.333 s, at `SidebackSpeed` | at its end to standing; to the walk if a movement key is down (at 0.45 of it, or at once); at 0.8 of it to the step in on the other side if `Step` asks for that |

The clips carry the step as root motion: 0.32 m aside and back. The body's ribcage, and so the eye, goes down 17 cm and a further 16 cm aside in the hold (recorded).

**What the movement state does.** All six animator states share one state object. Each animator state's script says which part it is (in, hold, out), which side, and a length; the part and the side are handed to the object each time one of them is entered, the length only once at the start, and what the object holds is 0.333 s.

- *Entered from another movement state:* the view's yaw is limited to within the trunk's limit less one degree (44 degrees) either side of the way the body faces, and the pitch to the standing range; the lean is set to 0; the progress of the step is 0 and its far end is 1 for right, -1 for left; `SidestepFloat` is set to 0 for the way in and to the far end otherwise.
- *Each further animator state of the six:* the progress goes back to 0 and the far end is taken from the new state's side. Nothing else.
- *Each frame:* everything standing does (the body follows the view past the trunk's limit, the animator's motion is applied with standing's carry-over, the animator's turning is applied). Then the playback speed below. Then `SidestepFloat`: the far end in the hold; on the way in `lerp(0, far end, progress / 0.333)`; on the way out `lerp(far end, 0, progress / 0.333)`. Then the progress grows by the frame's time on the player's clock. So the value is worked from the progress before this frame's time is added: the frame the state is entered gives 0.
- *Left for another movement state:* the yaw limit is lifted, `Step` is set to 0, and `SidestepFloat` is set to 0.
- The lean keys do nothing in it. Jump only takes the step back (`Step` 0, `SidestepFloat` 0). The movement keys work as standing: they raise `Inert`, and the animator leaves through the way out into the walk.
- Aiming is allowed in it. Its hard cap on speed is the walk's.

**`SidestepFloat`** picks, in the layer that bends the body with the view (`Additive_Aiming`), between the clips for a body stepped left (-1), standing (0) and stepped right (1). The walking state has code to run it down to 0 over `SIDESTEP_TO_MOVE_DURATION` (0.333 s) from whatever it stands at when a walk begins, but the sidestep's ways in and out have already set it to 0 on leaving, and the animator never leaves from the hold: the walk always finds 0. Not built.

**The playback speed** (`IdlePlayerInertia`; every standing-type state object has one of its own, and they all write the one animator parameter `SidebackSpeed`). Its resting figure is `TiltStartSideBackSpeed` evaluated at the inertia, times `InertiaBackwardCoef` at the inertia if the object last worked a way out. Entering a standing-type state from another kind sets the parameter to that. In the hold it is set to that each frame. On a way in or out it is `clamp(speed + TiltAcceleration(inertia) * dt, resting figure, TiltMaxSideBackSpeed(inertia))`. In this install `TiltStartSideBackSpeed` is 0.8 to 0.5, `TiltMaxSideBackSpeed` 1.2 to 0.8 and `TiltAcceleration` is 0 to 0, so nothing ever accelerates: the speed is the resting figure of the way in, 0.783 for the recorded character, on the way out too (the clamp only lifts a speed that is below the resting figure, and the way out's is lower). A step out or back therefore takes 0.333 / 0.783 = 0.426 s, while `SidestepFloat` finishes in 0.333 s.

**Recorded** (36, right then left): the state begins on the frame of the key; `SidestepFloat` climbs 0.024 or 0.027 a frame (8 or 9 ms over 0.333 s) and is at 1 after 41 frames; the player is 0.314 m aside after 0.35 s; on letting go the way out begins the same frame and standing 52 frames (0.433 s) later.

## Lean (`Tilt`)

- `Tilt` is -5 to 5. The smoothed value is what the animator and the weapon animation use.
- Normal lean (`InertiaSmoothTilt`) integrates a step multiplier:
  - while the target has just changed: the multiplier decays toward zero by `TiltInertia` per frame, and the new target is adopted once it is within `TiltInertia` of zero;
  - otherwise it accelerates by `TiltInertia` per frame toward the target (scaled by `InertiaBackwardCoef` when returning to centre), braking early by estimating the distance needed to stop; clamped to `±TiltInertiaMaxSpeed`;
  - `SmoothedTilt += 25 * multiplier * sign(target) * dt`, snapping to the target when within one step.
- In the move state, leaning into an obstacle is cancelled: if recent motion fell short of what was asked and the shortfall points toward the lean side, the lean is eased back to 0.
- Sprinting and jumping reset lean to 0.
- **The keys are presses and releases** (`Player.ToggleLean`, `Straighten`; read 2026-10-11). A lean key pressed sets the lean to its side, or to none if the character already leans that way; let go, it straightens the character if it still leans that way. Nothing holds the lean to the key in between. So a key let go where its release is not heard (under an open screen) leaves the lean standing, a second lean key pressed while the first is held leans the other way, and a lean put to zero by a sprint or a jump is not taken up again while the key stays down.
- **Which states listen** (`MovementState.SetTilt` and what overrides it). The request goes to the movement state in hand. Standing, walking, landing, lying still, going down and getting up, and the vault's states take it. A jump, a sprint and the stop after it, a step aside and a roll do nothing with it: the lean they put to zero as they began stays whatever the keys do. A crawl takes any request as one for no lean; it does not straighten a lean by itself as it begins, so a character that leans lying still and starts to crawl keeps the lean until a lean key is next pressed or let go.
- **Built** (2026-10-11, the session after the evening's; `Movement::ask_tilt`, `Input::lean_keys`). A host says how the two keys stand each frame (`lean_keys`, or `lean` for one of them); the simulation hears each change as the game's command, a release before a press where both come in one frame, and under a screen only notes them. Before, the lean followed the keys held, frame by frame, which straightened a lean the moment a screen shut and held a crawl's lean at zero. **Checked** (`the_lean_keys_are_presses_and_releases`, `a_lean_is_kept_under_a_screen`): the second key leans the other way and the first let go then does nothing; a sprint's zero stays with the key down and a press in the sprint is not heard; a lean kept under a screen stands after it shuts and the key pressed again straightens it. The recorded leans replay as before (`after26`). **From the code:** the recorder leans by the game's own two calls and the recorded leans agree, but none of those cases is on record. Not built: the slow lean (an axis of its own), and the base rule's easing of a lean into something that stops the character short.

## The slow lean (`Player.SlowLean`, `MovementContext.LegacySmoothTilt`; read and built 2026-10-12, night)

A second way to lean, on an axis of its own (`LeanX`; in the install the walk's sideways keys with `Left Alt` held, which then do not walk: `settings.md`).

- Every frame the game hands out its axes, after the frame's commands, the axis's push (-1 to 1) goes to `SlowLean`. Pushed, and with the body's lean within a degree of the lean asked for, it asks the movement state in hand for **a tenth of a degree more that way** (`Tilt + 0.1 * push`, kept to -5..5 by the lean's own limits), and notes that the lean was made slowly on this frame.
- **A frame, not a time.** The lean asked for goes 0.1 degrees a frame: 12 degrees a second at 120 frames a second, full lean in 0.42 s; at 60 frames a second half as fast. The game's own frame-rate limit is 120 in the install, and the simulation's frames are 120 a second.
- The body follows by the older rule, not the lean keys' (`LegacySmoothTilt`, for as long as the lean in hand is a slow one): each frame a share `dt * TILT_CHANGING_SPEED` (10, from the settings object) of what is left, and the last thousandth at once.
- **It stays where it is left.** Nothing straightens the character when the axis is let go.
- **The lean keys do nothing on a frame after one in which the lean was made slowly** (`_lastSlowLean`): neither a press nor a release. With the axis at rest, a lean key pressed is a lean of the ordinary kind again: to its side, or straight if the character leans that way already, by the inertia's rule.
- The request goes through the movement state like any lean's: a jump, a sprint, a step aside and a roll take none, and a crawl takes it as a request for none ("Lean", above).

**Built:** `Input::slow_lean`, `Movement::heard` and `smooth_tilt`; the app gives the axis as the bindings push it (`Bindings::axis("LeanX")`), every frame of the simulation. **Compared with the game: nothing**; no recording has a slow lean. Tried against the install (`a_lean_is_made_slowly_and_the_walk_key_puts_the_speed_aside`): twenty frames are two degrees, the lean stays, the lean key of that side straightens it, a lean key on the frame after a slow lean is not heard, and it stops at five degrees.

## The walk key (`Player.ToggleMinSpeed`, `RestoreSpeed`; read and built 2026-10-12, night)

`Walk` (`Caps Lock` in the install, bound as a press) puts the speed wheel to none and back:

- given, with the speed (as the state now limits it) above none: the speed as it stands is put aside (`_speedMemo`) and the speed changed by minus that, to none. A walk then goes at the floor under a walk's pace ("The floor's curve is shared"): 0.82 m/s for the unburdened character, against 2.82.
- given, with the speed at none: what was put aside is taken up again.
- taken back (the key coming up, where it is bound to be held): what was put aside is taken up again if the speed is at none, and nothing otherwise.

The change goes through the movement state like the wheel's, so a sprint takes none. What is put aside at first is the speed the character begins with. **Built:** `Input::walk_toggle`, `walk_end`. **Compared with the game: nothing.**

## Looking

- `Rotation` is (yaw, pitch) in degrees. Each look delta is scaled by sensitivity and the state's factor, limited in magnitude by the state's clamp, then added.
- **The state's factor and clamp** (`MovementState.ClampRotation`; built 2026-10-08). Each of the body animator's states that names a movement state carries two numbers, `StateSensitivity` and `RotationSpeedClamp`, and puts them on the movement state's object every time the animator enters it. The frame's turn (yaw and pitch together, after the mouse sensitivity) is multiplied by the first; if its length in degrees is then over the second it is cut to it; a clamp of 0 or less allows no turn at all. The clamp is per frame, so it follows the frame rate: 5 is 600 degrees a second at 120 frames a second. The sprinting state and the stop after it multiply both numbers once more on entering, by `Physical.SprintSensitivity`, which is 1 unless the character is overweight for sprinting and falls to `Stamina.SprintSensitivityLowerLimit` at the top of that range. From the install:

  | Animator states | Factor | Clamp |
  |---|---|---|
  | standing, its turns, walking, the standing sidestep | 1 | 99 |
  | the sprint | 0.5 | 10 |
  | the stop after a sprint | 0.2 | 10 |
  | a jump from a walk | 0.5 | 10 |
  | a jump from standing or from a sprint; the landing into a walk | 0.33 | 5 |
  | going down and getting up | 0.33 | 30 |
  | the dive from a sprint | 0.25 | 30 |
  | lying still, its turns, the roll | 0.33 | 5 |
  | the crawl | 0.4 | 5 |
  | the reload lying down | 0 | 0 |

  So the view turns at half the rate in a sprint and a third in a jump or lying down, and not at all through a reload lying down (until the new magazine is in: `firing.md`). Recorded (sixth recording, 28): a scripted 200 degrees a second for a second, lying still, turns the view 46.35 degrees, and 60 a second of pitch 13.89: the mouse sensitivity of 0.70 times 0.33.

  **Compared with the game** (seventh recording, whose recorder writes the state's two figures every frame, what the script asked the view to turn and what the game multiplies that by). The two figures are the game's on every frame of all 49 scenarios, which between them pass through every row of the table but the jump out of a sprint. Turned in, with the view then driven from what the script asked through the simulation's own limits instead of following the recording (`replay --own-view`): a sprint (44) lets through 0.3500 of what is asked, which is the 0.70 of the mouse and the 0.5, and cuts 17.5 degrees a frame to 10.000; a jump from standing (45) 0.2310 and 5.000; a jump from a walk 0.3500; the landing into a walk 0.2310; going down (46) 0.2310; the crawl (49) 0.2800 and 5.000; lying still 5.000. The view so driven is the game's within 0.0005 degrees at the end of every one of the eight scenarios that turn, and the movement state and animator's state stay the game's on every frame. The reload lying down shows its 0 and 0 on the recorder's frames, but the script's turn had ended a frame before it began. **The eighth recording** (56 scenarios) has the two figures right on every frame as well, the jump out of a sprint (57) now among them, and turns the view inside the reload lying down (55): on the 59 frames in which a turn is asked while the limit is 0 the game's view does not move at all, and the simulation's own view ends the scenario within 0.0001 degrees of the game's. Its two scenarios that pitch the view with the sights up, lying and crouched (53, 54), end within 0.0000: the aiming sensitivity goes through the state's factor like the plain one.
  **What a host shows between frames.** A host that draws more often than the simulation's frames come shows the mouse's travel at once, ahead of the frame that will take it. That travel has to be shown as the frame will let it through, by the state's factor and clamp and the limits below (`Movement::look_ahead`). Shown whole, each frame in a sprint, a jump or lying down takes part of it back, and the view shakes: at 144 drawn frames a second and a degree of mouse per drawn frame, a jump from standing stepped the view 1.0 degree forward and 0.34 back where it now turns 0.33 a frame (measured in the simulation, `the_view_shown_between_frames_turns_evenly`). The user reported it as a jittery view when turning in a jump (2026-10-09). This is the runtime's own matter: the game has one frame rate and nothing to show ahead of.

  **How the mouse's travel reaches the frames** (the same day, when the user reported that the weapon and arms still shook when turning in the air). A host's frames and the simulation's do not line up, so the travel gathered when one of the simulation's frames comes due covers anything from one of those frames to two. Given whole to that frame, the view turns by uneven amounts on frames of even length: by 83% of a frame's turn at 144 drawn frames a second, 73% at 165, nothing at 240, where the rates divide. The view as shown hides it, since what is shown ahead makes up the difference, but everything that follows the simulation's view with a lag does not, and in a jump that is the body and the weapon it carries. Each frame now takes the share of the travel that is its own time's (`pacing::Frame::input_share`) and the rest stays with the host for the next.

  **The body's turn in a jump, as drawn.** In a jump the body turns after the view by the player's whole-millisecond clock ("Jumping"), so on frames of even length it turns an eighth more on every third frame (9 ms against 8). That is the game's own, like the uneven travel through the air, and it shows where the game shows it: on the weapon, which the body carries in the air, and the arms. `Movement::drawn_body_yaw` is where the same turn has got to by the frames' own time, and a host draws the body's hold on the weapon, the arms and the ribcage by it (`drawn_body_in_view`); the simulation's own `body_yaw`, which the recordings are compared with, is unchanged. Measured in the simulation (`the_body_follows_the_view_evenly_in_a_jump`: a standing jump, the mouse at a steady 360 degrees a second, the view up to 67 degrees ahead of the body): the angle between body and view, frame against its neighbours, rippled by 0.93 degrees at 144 drawn frames a second and 0.98 at 165 with the travel given whole; by 0.28 with it shared by time, which is the game's clock alone; and by 0.004 as drawn. Not checked by eye: only the user can say whether the shake is gone.
- Pitch is clamped to the stance's range; yaw is unlimited standing.
- When the hands are turned away from the body by more than `TrunkRotationLimit`, the body turns to keep them within it, at `lerp(body, target, 30 * dt)`.
- After that the standing-type states turn the body by whatever the animator's clips turned the root this frame. That is nothing except in a turn on the spot, which the animator starts by itself when the view is more than 45 degrees from the body (`body-animator.md`, "Turning").
- **The angle is a frame old.** The angle from the body to the view (`HandsToBodyAngle`) is only worked out when the body is turned (`ApplyRotation`), which happens inside a state's update, after the animator has run. The standing states' test against the limit, the animator's `DeltaRotation` and its `Aim_angle` are therefore what the last update left: they lack this frame's turn of the view. Lying still is the exception: it takes the angle afresh before testing it. Recorded: on the frame the view passes 45 degrees from the body the body does not move, and the turn on the spot starts the frame after.
- **The angle is rounded, and a body held at the limit reads exactly the limit.** The game works the angle out as `Mathf.DeltaAngle(360 + body, 360 + view)` in single precision: the body's bearing read off its rotation as an angle from 0 to 360, the view's as it is kept (between -360 and 360), each with 360 added before the two are taken apart. A single-precision number between 512 and 1024 comes in steps of 1/16384 of a degree (0.000061), and half that between 256 and 512, so the angle is a whole number of such steps. The body's rotation itself is a quaternion, which resolves about a millionth of a degree. So the body's following of the view, which takes a share of the excess over the limit off each frame, does not stall a hair short: it runs on until the angle reads the limit itself, 45 and not 45.00002. That decides something, because the animator's condition for a turn on the spot is *more than* 45. A view that stops while a turn on the spot is still playing leaves the body drawn after it at the limit, and when the turn ends no further turn starts. Recorded (eleventh recording, 59: a reversal in mid-turn with the view 60 degrees down, the view coming to rest 0.45 s before the second turn ends): `DeltaRotation` reads exactly 45 on each of the 630 frames from 0.07 s before that turn's end until the view next moves, the animator goes to standing and stays there. With the angle taken as the plain difference of the two bearings it stood at 45.000015, and the simulation started a third quarter turn there that the game does not make (the body 90 degrees out for 3 s). Built: `math::body_to_view`, and the body's bearing kept finer than single precision while it follows the view (`Movement::turn_body`), as the game's rotation is.
- **So the eye trails the view by a frame.** The body's animation turns and bends the upper body to the view by those two angles, and the eye hangs on the ribcage. With the angles a frame old, the ribcage goes round the feet (and forward and down, for the pitch) one frame behind the view, and is at rest a frame after the view is. Recorded (tenth recording, scenario 11: the view turned at 84 degrees a second, the ribcage 0.13 m ahead of the feet): 1.6 mm short of the view sideways from the first frame of the turn to the last, and back on the frame after. With the view far down the ribcage is 0.44 m ahead and the same frame of trailing is 20 mm at 300 degrees a second. A host that shows the view ahead of the simulation's frames has to draw the ribcage for the view it shows, a frame behind it and no more: `Movement::posed_for` says which view the last frame's pose was made for and `ribcage_posed_for` gives the ribcage for another (`body-animator.md`, "Where the ribcage is drawn when the view turns").
- **A state's update can run twice in a frame.** The animator calls on the movement state once for each of its own states that names it and had a part in the frame: the state playing, one being blended into, and one left during the frame. Standing gives way to a turn on the spot with no blend, and both name the standing state, so on that frame the body's following of the view is worked twice, the second time on the angle the first left. Recorded on the three frames where it shows: 0.504, 0.0027 and 0.214 degrees turned where one pass gives 0.127, 0.0016 and 0.066.

## Settings sources

**`globals.json` `config`.** `WalkSpeed` and `SprintSpeed` are ranges; so is nearly every entry of `Inertia`. A range `{x, y}` "evaluated at t" means `lerp(x, y, t)`; "inverse" means `inverseLerp(x, y, t)`. In this install `WalkSpeed` is 0.625 to 0.717 and `SprintSpeed` 0.5 to 1.

**`EFTHardSettings`.** A settings object named `eftsettings` in the loose file `EscapeFromTarkov_Data\resources.assets`. That file carries no type trees, so the object is read with a field layout taken from the class definition (`crates/trp-data/src/hard_settings_layout.txt`): fields in declaration order, floats and ints 4 bytes, bools 1 byte padded to 4, curves as a key count, 28-byte keys (time, value, in slope, out slope, weighted mode, in weight, out weight) and three trailing ints. The layout stops before the first field whose element type has not been described (`ColliderPresets`).

At startup the game overwrites keys of four of its curves from `globals.json`: key 0 and key 1 of `SpeedLimitAfterFall`, `SpeedLimitDuration`, `InertiaTiltCurve` and `sprintSpeedInertiaCurve` are replaced by the matching `...Min` and `...Max` pairs (time = x, value = y, flat tangents). Whenever carried weight changes, key 1 of `MovementAccelerationRange` is replaced by the point interpolated between `MinMovementAccelerationRangeRight` and `MaxMovementAccelerationRangeRight` by `Inertia`.

**`Physical`** (from carried weight):

- `Inertia = lerp(n³, n, InertiaLimits.z)` with `n = inverseLerp(InertiaLimits.x, InertiaLimits.y, weight)`. The limits grow with strength by `InertiaLimitsStep` per level (0.025 kg here), and every overweight limit is times one and Strength's lift (`PhysicalBase.UpdateWeightLimits`): in the app since 0.103.0 (`settings::WeightLimits`, `Movement::set_weight_limits`, as the skill's level changes; `skills.md`), never in the replay, which reads an untrained character's weight off a recorded inertia. Here: 0 to 73 kg, mix 0.5.
- `SprintAcceleration = inverse(SprintAccelerationLimits, Inertia)`; `PreSprintAcceleration = SprintAccelerationLimits`' sibling `PreSprintAccelerationLimits` evaluated at `Inertia`.
- `MoveSideInertia = SideTime(Inertia)`, `MoveDiagonalInertia = DiagonalTime(Inertia)`.
- `SprintSpeed = lerp(1, Stamina.SprintSpeedLowerLimit, SprintOverweight)`.
- `WalkInertia = Inertia.WalkInertia(Inertia)`, `TiltInertia = InertiaTiltCurve(Inertia)`.
- `PoseLevelIncreaseSpeed` and `PoseLevelDecreaseSpeed`: `Stamina` ranges evaluated at `max(Overweight, 0.9 if exhausted)`.
- Overweight values are inverse lerps of weight over `Stamina` limit ranges. **Stamina itself is unread.**

## What is asked of a host's world

The simulation has no collision of its own beyond standing on the ground. It asks a host these things (`World` in `crates/trp-sim/src/movement.rs`), the middle three since 2026-10-09 and the later ones since 2026-10-10:

- the ground's height at a place, and which way the ground faces there (level unless the host says);
- where the ground holds the feet of a character at a place (`footing`): the ground's height under its axis unless the host says more. The ground check and the feet go by this. A host whose character has a foot with some width to it answers with where that foot rests, which over an edge it overhangs is on the edge: the game's capsule has a rounded foot 0.37 m across, and a climb relies on it (`vaulting.md`, "The clips");
- whether what a line hit at a point is the terrain itself and not something standing on it (`terrain`): terrain is never vaulted or climbed. Everything is terrain unless the host says otherwise, so a world of bare ground vaults nothing;
- how far along a line the first solid thing lies (`line`): the game's line cast, which the weapon feels for obstacles with (`weapon-handling.md`, "Turn-away"). A host that says nothing gets the ground alone, found from its height, and hit from above only, as the game's ground is;
- where a move from one place toward another leaves the character's feet (`obstructed`), asked once for every move the simulation makes, before the ground is put under the place answered and before the speed is worked out from what was moved. It is told how much of the upright space over the feet the character's collision takes up (`Capsule`: from the feet to the stance's height, but in a vault or a climb from well above the feet, `vaulting.md`, "The capsule"), so that it can let the character past what is lower than that. The place asked about is where the motion alone would put the feet, which may be under the ground there; a rise too high to step onto is therefore the host's to refuse (since 2026-10-09: the ground used to be put under the feet first, which lifted a character onto anything it walked into). A host that says nothing obstructs nothing;
- whether that answer has already set the feet on the ground the character moved onto or into (`sets_feet`, since 2026-10-11). A host that says nothing has its character's feet lifted onto `footing` after every move; one that keeps the capsule out of things the game's way says yes;
- how far a box has to move to be clear of everything solid (`push_out`): for each thing the box is in, the shortest move that takes it out, a margin longer, added up. The game keeps the first-person camera out of the world with it (`weapon-handling.md`, "The camera's collider"). A host that says nothing gets the ground alone.

**The ground a host reports is the ground itself.** The character may stand over it by up to its ground check ("What counts as on the ground"), and what is cast into the world or pushed out of it (the weapon's line, the camera's box) meets the ground, not the level of the feet.

**The character's capsule**, for a host that wants to answer the last as the game would. `CharacterControllerSpawner` on the player prefab: radius 0.366 m, height 1.6 m, centre 0.91 m up, a skin of 0.1 m, a step of 0.2 m, slopes up to 50 degrees. The spawner makes one of several controllers by a mode; the engine's own, which takes all of those figures, is one of them (`trp_data::player::CapsuleData` reads them). Which mode the local player gets and how the controller slides along what it meets are not read. The push-out from obstacles (`POM`, the player overlap manager) is read since the eighth recording: its camera half is built (`weapon-handling.md`, "The camera's collider"); its other half, a capsule about the head that pushes a move or a turn lying down back out of what the head would enter, is not. The app's test range keeps the character out of its boxes by the rules of the next section, with the radius alone.

## The character's own collision (`SimpleCharacterController`; read 2026-10-10)

**Which controller.** The local player's controller is not the engine's. `ApplicationConfig`'s `ClientPlayerMode` is `ControllerType.Simple`, and for that type the spawner makes the game's own `SimpleCharacterController` over a capsule collider. Of the spawner's figures it is given the step and the slope limit; the skin is left at nothing. Recorded (thirteenth recording): the capsule's centre standing is 0.8 m up for a height of 1.6, which is `0.8 + skin` with no skin, and a character walked into a wall stops with its axis 0.37 to 0.39 m from the face, the radius (0.366) and not the radius and a tenth. So the capsule is the character: from the feet to the stance's height, 0.366 m to every side, with a rounded foot and head.

**A move** (`Move`, once for every motion a movement state applies; a frame's motion is far under the 0.3 m the controller cuts long moves into, so each frame is one step of it). The capsule is put where the motion would take it, and then, for every solid thing near it that it now overlaps, in the engine's order, it is moved out of that thing by the shortest way (`Physics.ComputePenetration`: a direction and a distance), each from where the one before left it. With the way out's upward part written `up` (the direction's dot with straight up):

- `up` of 0.087 or more (within 85 degrees of straight up): the character **touched ground** in this move. That is one of the two things that make it count as on the ground in the next frame (the other is the ray under its axis, "What counts as on the ground").
- `up` of 0.64 or more (within about 50 degrees): the way out is taken **straight up** instead, further by `1 / up`. So ground pushes the feet up and never sideways, a gentle slope does not slide the character down, and the corner of a low step lifts the rounded foot over it: a step up to 0.36 of the radius (13 cm) is ridden up as it is walked into.
- anything flatter pushes the capsule the way it points: back from a wall, and back and up from the corner of a higher step.
- **In a vault or a climb the upward part of every way out is dropped** (`SetEnableVerticalDepenetration(false)`, put back when the move's state is left). A way out already turned straight up then does nothing at all, and a slanted one pushes back by its level part alone. The character rises by the move's motion only, and passes into what its capsule's underside has not yet cleared.
- If the whole of the step (motion and ways out together) comes to 0.2 m or more, it is cut to 0.05 m along the same line. A capsule that finds itself well inside something comes out of it 5 cm a step.

Then the frame's velocity is taken from where the step ended, and the hard cap on speed ("The hard cap on speed") cuts it and the step if it is over.

**What follows from it, recorded** (thirteenth recording, the four vault traces):

- *After a vault the character is pushed out of the wall it vaulted.* The vault's state ends with the axis some centimetres past the wall's far face and the capsule, shrunk for the move, is put back to its full size inside the wall's reach. It comes out forward at the step's 5 cm, which the state's cap on speed (4.27 m/s for the recorded character) cuts to 3.6 cm a frame: nine frames at exactly that pace, 0.32 m, until the axis is the radius clear.
- *A walk off a top begins to fall as the axis passes the edge.* On the top the capsule touches and does not overlap, so nothing counts as touched ground; once the ray under the axis finds nothing within the ground check, gravity has it, and at a walk's pace it is carried clear of the corner before it has fallen onto it. (At a crawl's pace the foot would sink onto the corner and be held there.)
- *A climb ends with the foot on the corner.* The climb's state is left with the axis short of the top's edge; the capsule comes back to full size with its rounded foot a little into the corner, is pushed straight up out of it, and counts as on the ground by that touch, frame and frame about, until a step forward brings the axis over the top.

**One more thing a move does** (`ResolveUnityDepenetrationBug`, at the end of every move): a ray straight down from the middle of the capsule's rounded foot *where the move began*, as long as the radius less a millimetre. If it meets something, the feet are put on that and the move is dropped. A capsule that is pushed up out of the ground every move never has anything there; one that is not (a climb begun on rising ground) has every few frames. The ray starts inside the capsule, so it finds only what is no higher than that middle: a wall the axis stands inside of is not found.

**Built**: `crates/trp-sim/src/collision.rs`, in two forms with the same rules. `Course` is a world that is one line of ground with steps in it: what the replay rebuilds from a recorded path, and what the tests set walls and boxes up in. `Site` (2026-10-11) is level ground with upright boxes on it, with width as well as length: a box's side stops the capsule its radius off and lets it along, an upright edge is rounded by the radius, a top no higher than 0.36 of the radius is ridden up, a rim holds a foot that overhangs it. The app's range answers `World::obstructed` and `World::held` with it since that day (`crates/trp-app/src/range.rs`). `World::obstructed` is told whether the capsule may be pushed upward (`Capsule::lift`), `World::held` asks a host whether a character rests on something beside what is under its axis, and `World::sets_feet` says whether the host's move has already put the feet on the ground (the game's controller does, by pushing the capsule up; a host that says so is not then lifted onto whatever is under its axis, which is what put a character that had just vaulted on top of the wall for two frames in the app before). A host with a collision engine of its own answers all three from that; the rules above are for one that wants the game's behaviour at a wall, a step and an edge.

**In the app, measured 2026-10-11** (the app's own log of what it draws, `--log-pacing`): a vault over the range's knee-high wall ends with the axis 4.7 cm inside the wall's far face and the character is moved out at 3.56 m/s (the 5 cm a step, cut by the state's cap) until the axis is the radius clear, 0.41 m, and then walks on; a climb onto a block ends 4 cm under the top with the foot on the rim and is pushed straight up; the stairs (24 of 0.1 m since that day: 14 of 0.167 m would have been pushed back as well as up) are walked up at 2.83 m/s, the walk's own pace; and a walk off the platform falls from the frame its axis passes the edge. None of that is compared with the game beyond what the four vault traces already showed of the same rules.

Not built: "feeling tight" between two things that push opposite ways (nothing in the range stands that close), seeping through other characters, the controller's unused step-up, and boxes that do not stand on the ground (a box is solid from its top down unless the capsule is wholly under it).

## Stopped by what stands in the way (`ObstacleCollision`; read 2026-10-10)

A character that pushes into something and gets nowhere is made to stop pushing. The game measures how fast the character really moves (its place this frame less its place the frame before, over the frame's time) and keeps a lock:

- **Put on** in the walking state at under 0.2 m/s and in a sprint at under 0.5, looked at each frame the body animator's first layer is not between two states: the character has to be that slow, its movement direction longer than 0.9, and to have been that slow for more than 0.1 s of such looks. The direction it was put on with is kept.
- **While it is on** (`MovementContext.CanWalk` is false): the walking state lowers `Inert`, so the walk runs out into standing; the sprinting state lowers the sprint's flag (and `Inert`, with no sideways key), applies no motion, and the sprint ends through its stop; the standing state sets the tiny starting direction as ever and then starts nothing; and a vault out of a sprint does not ask for the sprint again.
- **Taken off** when the movement direction comes to point more than 5 degrees from the kept one, or the view is turned more than 5 degrees from the body; and by entering a jump, a vault or a climb.

Recorded (thirteenth recording, the sprint at a wall too high to vault): the sprint is stopped dead at the wall for a quarter of a second, goes into its stop and on into standing, and the climb the held key gets there starts on the standing state's first frame. Built: `Movement::blocked` and its neighbours in `movement.rs`.

## Slopes and sliding (`MovementContext.UpdateGroundCollision`, `DetectSurfaceNormal`, `ProjectMotionToSurface`, `ApplyGravity`; read and built 2026-10-25, 0.130.0)

**Which way the ground faces.** Every frame, in the ground check, the game works out a surface normal for the character (`SurfaceNormal`). The ray straight down the capsule's axis that tells whether the character is on the ground looks a quarter of a metre further than the check itself (`additionSearch`), and reports the normal of what it meets. The character's own controller reports another: of the ways out of the ground it pushed the capsule along in its last move, the one that points most upward, among those within 85 degrees of straight up (`surfaceNormalAccordingToCapsule`; straight up if the capsule was in nothing, and all three figures nought if it was in walls alone). The surface is the ray's normal where the ray found ground that is not too steep (an upward part of 0.65 or more, about 49 degrees); where the ray found steeper ground, the flatter of the ray's and the controller's; where the ray found nothing, the controller's. On steep ground the ray often misses: the capsule's rounded foot rests on the slope well to one side of where the axis meets it.

**The average.** The surface's upward part is kept over the last five frames (`NormalDotHistory`, a `RollingMedian` of five whose `Avarage` is the plain mean of what has been added, up to five). The two rules below go by that average, not by the frame's own figure.

**The slide.** Where the average is under 0.65 the character stands on ground too steep to stand on, and the game works out a pull (`Slope`): the way up the slope, square to the ground (the cross of the cross of the surface and straight up with the surface), as long as 0.9 less the frame's own upward part. Where the average is 0.65 or more the pull is nothing. Any state that sticks to the ground (all but the jump) then adds, in its gravity step, the pull times gravity's downward part (-9.81) and the frame's time, in place of the usual fall: down the slope at 0.9 less the upward part times g metres a second, 2.5 m/s at the threshold and 3.9 m/s at 60 degrees, steeper ground faster. A long fall (a quarter second off the ground) takes precedence and is handled as before. The hard cap on speed still applies, counting the level part only when going down, so a character standing on such ground slides at the standing state's cap, an untrained one at 0.5 m/s on the level (a walk's or a sprint's cap is higher). The controller's own push out of the ground adds to it on steep ground: steeper than 50 degrees it pushes along the ground's normal rather than straight up ("The character's own collision"), which the simulation leaves to the host. Standing on another character gives a pull of its own (`HumanPyramidSlopeLength`, off their head); not built. A speed limit named after the surface normal exists in the code and is never anything but 1.

**Motion laid down a slope.** In the states that take the base motion (standing, walking, sprinting, the sprint's stop, the landing, crawling and getting up; not lying still, rolling, going down, the jump, a vault or a climb), the frame's motion is laid along the ground before gravity (`ProjectMotionToSurface`) where the average upward part is above 0.5 and under 0.995 (from about 6 to about 60 degrees): the motion's heading along the surface is found, and if that heading goes down, and at least half the motion lies along it, the motion is put on it and lengthened so that its level part stays as it was. Going up, across a slope at more than 60 degrees from its fall, or on ground nearly level or too steep, the motion is left alone. So a walk down a slope keeps to it instead of stepping off into the air and riding the ground check down, and a walk up it is pushed up by the controller as before.

**The slide out of a sprint** (`SprintPlayerState.ChangePose`, `ManualAnimatorMoveUpdate`, `MovementContext.SetPoseLevel`). Crouching in a sprint is the only change of stance the sprinting state takes: it marks the sprint as sliding, lowers the animator's `Inert` flag and puts the stance to a full crouch *by force*, which also puts the smoothed stance there at once: the body and the camera are crouched on that frame, not eased down. From the next frame, as long as the sprinting state lasts, the sliding mark lowers the animator's sprint flag and `Inert` whatever keys are held, and applies no motion, as when forward is let go. The sprint itself (`Physical.Sprinting`) is switched off only as the state is left. The body animator answers `!Sprint & !Inert` by going from the sprint to its way out of it (`Transition_Sprint_to_Stand`), whose clips are chosen by the stance (`Level`): at a full crouch, `Transition_Sprint_To_Idle_Low` and its fast form, so the slide is the crouched way out of the sprint, carried on by the sprint's stop as that state keeps the sprint's motion ("A frame with no input"). Built: `Movement::change_pose` and `update_sprint` in `movement.rs`. No recording has a crouch in a sprint: this is from the code alone, `PROVISIONAL` until one does (a sprint on level ground with the crouch key pressed in it, by hand or in a course).

**Built**: `crates/trp-sim/src/movement/slope.rs` (the surface, the average, the pull and the laying), `Movement::feel_slope`, `apply_motion` and `fall_and_move` in `movement.rs`. A host answers the controller's half with `World::contact_normal`; left alone, a world that is only ground gives its normal under the axis where the feet are on it. The app's range is level, so nothing in it slides; the replay's grounds lean 0.22 degrees at most, under the laying's 6. The pull acts whatever the options' MOVEMENT says, as gravity does (the switch takes the keys away, not the ground); the slide out of a sprint goes with the movement's keys.

**Compared with** `courses2\6-slope-steep` and `6-slope-gentle` (recorder 0.10.0, by hand, 2026-10-17; 7,279 and 3,970 frames), rule by rule on what the game recorded going into each (`slope` binary, `crates/trp-sim/src/bin/slope.rs`; `local\slope_compare_s135.txt`): the five-frame average on every frame of both (7,278 and 3,969) to 0.000000; the surface from the ray's hit on 6,693 frames to 0.000000 (61 with a steeper hit, where the surface was never steeper than the hit; 585 with no hit, where the controller's figure, not recorded, is the surface); the slide's pull on every frame to 0.000000, 158 of them sliding in the game (all in the steep recording); the motion laid down a slope on 1,245 frames of the steep recording within 0.0006 m/s of the game's rise and on 471 of the gentle one within 0.000001. What was not compared: a whole slide or walk down a slope against the game's path (a recording by hand has no script, and only its path's ground is known), and the slide out of a sprint (no recording has one).

## The head's push out of walls (`PlayerOverlapManager`, `ColliderExtrusion`, `MovementContext.LimitMotionXZ`, `RotationOverlapPrediction`; read and built 2026-10-25, 0.132.0)

Besides the character's own capsule, the player prefab's `PlayerOverlapManager` keeps a capsule about the head (the other half of the manager that keeps the camera's box out of walls: `weapon-handling.md`, "The camera's collider"). The movement asks it how far the frame's motion would take the head into something, and pushes the motion back by that, on the level only.

**The two capsules** (the prefab, `trp_data::player::HeadCollidersData`). Standing (and crouched, walking, in a sprint, a jump, blind fire, a mount): on `playercollider`, a child of the camera's container at 0.15 m up and 0.15 m ahead in the container's axes, turned with it; a capsule of radius 0.22 m and height 0.7 m along the container's up, its middle at (0, -0.2, -0.1) from its object: so its middle 5 cm under and 5 cm ahead of the container, tipping with the view's pitch. Lying: on `ProneCollisionTest`, a child of the player's animated object; radius 0.175 m, height 1.6 m along the body, its middle 0.15 m over its object; the object is moved, whenever the push is asked for, to the second spine bone's place in the animated object's axes, no higher than 0.2 m (`UpdateProneColliderPosition`). The lying one is put in use on entering going down, lying still, the crawl and the roll, and the standing one on leaving the crawl, the roll and getting up (`SetPOMCollider`): so getting up out of lying still has the lying one to its end, and out of the crawl the standing one. Going down nothing asks for the push, so the lying capsule stays where it was last put.

**The push** (`ColliderExtrusion.Calculate`). The capsule is put where its object is plus the frame's motion; every collider near it on the low-poly world's, the doors', the terrain's and other players' layers (the head's own colliders left out) is asked for the shortest move that takes the capsule out of it; each is lengthened by a millimetre, and they are added up, all from the same place (not one after another as the camera's are).

**The limit** (`LimitMotionXZ`, in the states that take the base motion, the jump, the roll and lying still; not going down, nor a vault or a climb): the push's level part, if its square is at least 0.0001 (lying still, 0.0009: a push under 1 cm, or 3 cm, is let be), is held to the frame's motion's length and a centimetre, never less than 5 cm, and added to the motion. It is worked out after the slope's pull and gravity and before the crawl's turn back ("Room to move"). Walking straight into a wall the head is not what meets it: with the view level its front is about 2 cm behind the body's capsule's. It reaches past the body when the body leans, when the view looks down, or round a corner, and then it carries the body off the wall (a lean into a wall pushes the character away from it).

**A turn** (`RotationOverlapPrediction`): the capsule's object carried round the player's axis by the turn and moved by a shift, turned upright to the view's yaw with the animated object's pitch and roll and the turn (not to the camera's pitch), asked for its push. A lying body's turn (`IsAbleToRotateProne`, with the turn doubled) is dropped if the level push's square is 1e-6 or more ("Room to move" has the ground's half); a mounted weapon's turn outward (`IdleWeaponMountingState.Rotate`) if the level push is anything at all (`mounting.md`, "The turn").

**Built**: `crates/trp-sim/src/movement/pom.rs` (`Movement::head_capsule`, `head_push`, `limit_motion_xz`, `head_turn_push`; `pom::on_container`, `over_pivot`, `limited`), called from `fall_and_move`, the jump, the lying turn and the mount's turn; `World::capsule_push` for a host (left alone, the ground under the capsule's ends; the app's range: its boxes, each the deepest of spheres a centimetre apart along the capsule's line, `PROVISIONAL`, and the ground; the replay's: its terrain and course 8's sill). `Body::spine2`. Without the prefab's capsules (settings made without the install) nothing pushes the head and the mount asks the host's `World::turn_blocked` as before. Under the options' MOVEMENT, as the motion it limits.

**Compared with** `courses2\6-head-walls` (recorder 0.10.0, by hand; 5,774 frames) and `courses7\10-anything`'s first take (recorder 0.14.0; 8,373 frames, mounted at a window sill), on the game's own figures (`head` binary; `local\head_compare_s135.txt`): the standing capsule's object worked back from the recorded camera (the container: the eye less the camera's own place, turned as the camera's world turn less its own) to the recorded `head_capsule` on 13,299 frames within 0.002 mm; the lying one (the second spine bone from the first's recorded place and turn, 0.1624 m along it) on 781 frames lying still, crawling and getting up, within 4.9 mm on average and 3.2 cm at worst (unexplained: when in the frame the game places it, and the body's lift off the ground, which is not recorded); going down 0.20 m apart, as the game does not move it then; the limit, the recorded push added to the recorded motion before it beside the motion after it, on 1,240 frames with a push to 0.000000 m and 12,433 without to 0.000000 m but for frames whose before-limit figure the state leaves stale (in `courses7`, one of 2 cm). Not compared: the push itself, which needs the walls (the game's meshes are not readable, so the recorder wrote their names and bounds but no triangles); the head's lying turn and the mount's turn against a recording. The regression (`after166`) is `after165`'s but for the new `head_push` line, nought in both in the M60E6's recordings.

## Room to move (`MovementContext.OverlapOrHasNoGround`, `HasGround`, `LegacyTwoPointSteadyGroundCheck`, `CanRoll`, `CanProne`, `CanStandAt`, `LimitProneMotion`, `IsAbleToRotateProne`; `Transit2ProneState`; read and built 2026-10-25, 0.131.0)

Before a step aside, a roll, lying down or standing up, and while crawling, going down or turning a lying body, the game asks whether there is room: a box where the body would be, and rays down for ground it can stand on. Every figure below is the game's code's, but for the roll's three, which are the install's settings object's (`probe hardsettings Roll`).

**Steady ground** (`LegacyTwoPointSteadyGroundCheck`). A ray straight down from a point, for a given length: if it meets ground whose upward part is at least 0.707 (about 45 degrees), the ground is steady; if not (or if it meets nothing), a second ray 0.1 m further along the body's forward is tried the same way. Water counts as no ground at all (there is none here). The checks below send their rays from the capsule's middle level, as far down as half the capsule's height, 0.3 m, and what the slope adds (1 less the surface's upward part), plus whatever a check adds.

**A box and steady ground beside the capsule** (`OverlapOrHasNoGround`): a box out from the capsule's side toward a way, its middle at the capsule's middle's height, so deep, so wide either way of its middle (half the radius if not said) and the capsule's height over some figure either way of its middle, turned as the body is; refused if anything is in it, or if there is no steady ground under its middle. `HasGround` is the second half alone.

**The step aside** (`IdlePlayerState.SetStep`, which blind fire's and the sidestep's own states share but on the way back): a box 0.3 m deep to that side, 0.2 m across either way and a third of the capsule's height either way (so standing it starts 0.27 m over the feet: a kerb does not stop it). Refused, the step asked for is nought: nothing happens.

**The roll** (`CanRoll`, the lying still state's and the roll's own `SetStep`): a point beside the ribcage's height over the feet, 0.5 m to that side and 0.2 m ahead, turned to the view's yaw and the ground's pitch (`RollCheckShift`); steady ground within 0.5 m under it (`RollCheckGroundCastLength`), and nothing in a box there 0.25 m across, 0.1 m high and 2.1 m long (`RollCheckSize`). Refused lying still, the step is nought and the lean is left as it was; in the roll, nought.

**Lying down** (`CanProne`, the prone flag's setter; health's part as before, `health.md`): not on ground steeper than 30 degrees from level (the surface normal, "Slopes and sliding"); spheres of 0.25 m from 0.4 m over the feet swept 1.3 m ahead and behind along the ground, what each meets first counting at most 1.55 m and nothing counting 1.55; at least 0.5 m ahead and three times the frame's animated motion forward, and at least 1.7 m ahead and behind together; then two points, ahead and behind (placed by what the spheres found: behind by up to 0.6 m less a shift of 1.1 less the room behind, ahead by up to 0.4 m and that shift), each with ground within 0.6 m under it. Pressed in the air, the flag is asked for there and then, so the points are too high off the ground and the press is refused unless the character is within about 20 cm of it; at the jump's end the flag, if up, is asked again.

**Standing at a stance** (`CanStandAt`, in `SetPoseLevel`, which every change of stance goes through, the mount's own included): lower than the character stands now, and not lying, is always room; else a box over the feet, 0.21 m out to every side, for a height from 0.875 (crouched) to 1.15 (standing) by the stance, its middle at three quarters of that height and half as tall as five quarters of it (so crouched it reaches 1.20 m over the feet, standing 1.58 m), turned to the body's yaw; with medicine at work lying, never. Other characters count, the character itself not. **Getting up** asks it at the stance held lying (a full crouch), and failing that for a crouch (the same box here): with neither, the character lies on. Up, the stance last chosen with the stance keys is asked for, and taken only if it has room: under a ceiling between 1.20 and 1.58 m the character gets up crouched and stays so.

**Crawling and lying still** (`LimitProneMotion`, after the obstacle push): the frame's motion across the body is turned back if there is no steady ground 0.1 m out from the capsule's side that way, its motion along the body if there is none 0.6 m out along the ground that way; each ray reaches 0.15 m further. Lying, the capsule's middle is 0.35 m over the feet and its height 0.3 m, so the ground has to be within 0.6 m under that, about a quarter metre under the feet, and the body stops about 0.67 m short of an edge ahead (0.1 m more with the second ray).

**Turning a lying body** (`IsAbleToRotateProne`, the lying still state's, the crawl's and the roll's `Rotate`): with the view turned this frame, the body as twice the turn would leave it needs steady ground 0.55 m ahead and 0.75 m behind, each ray 0.15 m longer; if not, the frame's turn of the yaw is dropped (the pitch is not). The other half of the check, whether the head would swing into something, is the head's push out of walls (`POM`).

**Going down** (`Transit2ProneState.ProcessAnimatorMovement`): with a box 0.75 m deep ahead (half the radius across, a quarter of the capsule's height either way, its rays 0.2 m longer) in something or over no ground, the view's yaw is put back to the frame's start and the motion along the body is replaced by a push backward, eased at 10 a second toward how far the line from the capsule's middle forward goes into something within the radius and 0.35 m (at least a centimetre); failing that, the same behind (0.25 m across, rays 0.15 m longer; 0.4 m; 7 a second), pushing forward. The push eased so far is kept from one going down to the next. No other motion limit applies in that state.

**The vault's hands** (vaulting spec, "What a try does"): medicine or food in hand, or a grenade's throw held or under way, let only an obstacle under the chosen move's height without hands be vaulted, and nothing at all when no move would be chosen from the last measurement (`VaultingGameplayRestrictions.CheckMedsAndFoodOperation`, `CheckGrenadeOperation`); a host tells it (`Input::hands_limit_vault`). An item being moved in the inventory (`CheckInventoryOperations`) is not built.

**Built**: `crates/trp-sim/src/movement/room.rs`, with `Movement::set_step`, `prone_command`, `set_pose_level`, `fall_and_move` and the frame's turn in `movement.rs`; a host answers `World::box_overlaps` and `World::sphere_cast` (left alone, a world that is only ground: a box corner under it, a sphere touching it felt every centimetre) and the existing `World::ray` for steady ground. The app's range answers them from its boxes and its ground (the sphere tried every centimetre, `PROVISIONAL`). All of it under the options' MOVEMENT, whose keys it answers.

**Compared with**: nothing. No recording has a refusal: every kept scenario passes every check, and the regression (`after165`) is the last run's in every figure. `PROVISIONAL` until a recording by hand has them: a step aside into a wall and over an edge, lying down facing a wall half a metre off and on a slope over 30 degrees, getting up under a low ceiling, a roll beside a wall, a crawl to an edge, a turn lying at an edge, going down against a wall (recorder 0.14.0's `+` course would write all of it, the solids about the player too). Tests: five in `tests/sim/room.rs`, one in `tests/sim/install.rs` (the vault's hands).

## Not covered yet

Vaulting and climbing have a spec of their own (`vaulting.md`, built 2026-10-10). Not covered: standing on another character ("Slopes and sliding"), the harm from a fall and what the capsule does at an edge ("Falling"), stamina, and a swamp's speed limit ("Speed limits": the range has none).

## Compared with the running game

From traces recorded with the plugin in `hosts/spt-oracle` on 2026-10-07 (stock AKM, a character with Strength about 40, 7.5 kg carried by the inertia it shows, 120 frames a second). The replay starts each scenario as the recording started it: the wheel, the stance, the speed parameter, the compass bearing of the look and of the body, and what the walking state still held from the walk before (below).

What the traces settled:

- **Skills set the top speeds.** `MaxSpeed = WalkSpeed` and `SprintingSpeed = SprintSpeed`, each a pair in `globals.json` blended by `Strength level / 60`: walking 0.625 to 0.717, sprinting 0.5 to 1. The sprint parameter's limit is `SprintingSpeed + 1` for an unburdened character; the recorded one reached 1.83, which is Strength 40.
- **The speed parameter does not start from zero.** It starts at the selected speed and standing still leaves it where it was (a little lower after each stop). It only follows the wheel while moving: after the wheel is turned to the bottom at rest, the first steps are still at the old pace and slow down over about 1.5 s. The simulation now does both.
- **Sprint pressed before moving does nothing.** With sprint held first and forward pressed after, the game walks. The earlier guess that the press is repeated when movement begins was wrong and is gone. The user confirmed it by hand with the real keys on 2026-10-07: sprint starts only if forward is held before sprint is pressed.
- **Footsteps** upright come every 0.38 s, four per cycle of the locomotion blend.
- The game calls the walking state `Run`.

How close the simulation was after that first pass, moving by the root motion table with instant changes of state: the gaits within about 4%, the speed and sprint parameters within 0.01 and 0.05, a 3 s walk ending 0.46 m apart because the game takes 0.2 s to get going and the simulation took none, and a reversal turning round at full pace where the game drops through standing.

**With the body's animator running** (third recording, same day, the recorder resetting the wheel and stance before each scenario; the movement stepped on the recording's own frames). Root mean square and largest gap over each scenario:

| Scenario | Distance (over) | Movement state | Eye position |
|---|---|---|---|
| Walk forward 3 s | 1 mm, 2 mm (9.3 m) | the same on every frame | 2 mm, 2 cm |
| Strafe right, back, left, then forward-right | 2 mm, 3 mm (5.5 m out, 28 m walked) | the same on every frame | 1 mm, 2 cm |
| Forward, then back | 2 mm, 2 mm | the same on every frame | 2 mm, 2 cm |
| Slowest walk | under 1 mm (3.2 m) | the same on every frame | 2 mm, 2 cm |
| Crouched walk | under 1 mm (4.5 m) | the same on every frame | 2 mm, 2 cm |
| Sprint | 8 mm, 12 mm (23.6 m), all of it at a bump in the ground | the same on every frame | 3 mm, 2 cm |
| Sprint key held before forward | under 1 mm (10.8 m) | the same on every frame | (slope) |
| Walk while turning | under 1 mm (13.3 m) | the same on every frame | (slope) |
| Standing jump | none | 4 frames apart at the end | 2.8 cm, 19 cm |
| Lean | none | the same | 3 mm, 2 cm |

In all of these the speed parameter agrees within 0.001, the sprint parameter exactly as far as it is printed, the direction within 0.009, and the velocity figure within 0.01 m/s apart from two places: one frame as the walk resumes in the reversal, where nothing moves on either side and the game's figure still holds the frame before, and the bump in the sprint. Before the corrections below the first three rows stood at 4, 13 and 6 cm, the sprint at 3 cm, and the velocity figure 0.12 to 0.2 m/s apart throughout.

**What the walk's start showed.** The first 0.4 s of a recorded walk used to come out up to 7% slower than the simulation's (2.73 m/s against 2.92 as the blend into the walk ends) with the speed setting the same on both sides. Three things were behind it, all in the game's code and now in the simulation or the replay:

- **The walking state keeps the last walk's direction rate** ("What entering does not clear" above). The simulation leaves 4.38 to 4.40 per second at the end of each recorded walk, pointing back along it. The recording shows the same 4.38 to 4.40 on the first walking frame of each scenario that follows, once that frame is counted as the 8 or the 9 ms the player's clock made it (which of the two follows from the frames after it). With the rate carried, the direction over the first half second of a walk agrees within 0.009 where it differed by 0.16, and the distance within a millimetre through the blend.
- **On the walk's first frame the smoothing aims at where it stands**, so the simulation's direction had been one frame ahead.
- **The first scenario started with the body 1.8 degrees off the look**, left by the player's own turn before the recording, and the body's turning to the look bends the direction a little while it lasts. The replay takes the body's bearing from the recording.

The replay cannot know what the scenario before left in the walking state, since it plays each one alone, and the first scenario follows the player's own walk to the spot. It reads the rate off the recording's first walking frame, where it is all that moves the direction, and prints it beside the rate its own replay ends with for comparison with the next scenario's. Nothing moves on that frame, so the player's clock does not show on it and the engine's time is used; the rate read is 4% low or 7% high for that reason. Where the 1 to 2 mm left in the first three rows come from has not been traced.

**What the sprint showed.**

- **The player's clock** ("The player's clock" above). The motion over the game's velocity figure comes to 8.0 or 9.0 ms on every frame in which the recorded player moves. The replay steps the animator by the recorder's frame time and the movement states by that figure. With it the velocity figures agree, and so does the 0.1 s after the sprint key is let go, where the walk's hard cap of 4.27 m/s lets through 4.27 m/s by the game's own count on two frames in three and whatever is left of the sprint's motion on the third: on a single clock the simulation held 4.27 m/s flat there and gained 8 mm.
- **The compass bearing** ("Moving", on entering). The speed parameter after the sprint agreed only once the replay faced the recorded bearing (171 degrees, nearly along -z). Facing +z the simulation's stayed up to 0.011 above the game's for half a second (0.998 against 0.990 after one frame).
- **What is left is ground.** Through the walk's start, the wind-up, 17 m of sprinting, the change back to a walk and the stop the two are never more than 0.3 mm apart, except that the recorded player ran up a 1 cm bump in three frames and lost 11 mm there.

- **Starting and reversing** go the game's way, because they are the animator's blends: see the figures in `body-animator.md`.
- **Stopping** goes the game's way because of the hard cap on speed above. Before the cap was in, the simulation coasted through the blend out of the walk and ended 0.19 m further on.
- **Pace.** Upright at the top of the wheel the positions advance at 2.96 m/s half a second into the walk, rising to 3.01 by its end, in both. The earlier figure of "2.75 to 3.15 within each stride" was a misreading: the game's own velocity figure alternates 3.12, 3.12, 2.78 from frame to frame at 120 frames a second while the positions advance evenly. There is no swing of pace within the stride to reproduce. The alternation is the player's clock in whole milliseconds ("The player's clock" above), and with the replay on that clock the two figures agree.
- **The recorder's keys are a frame late.** The game reads its movement keys before the recorder's clock moves on, so a recorded walk starts and stops one frame after the script says; the recorder's other commands (sprint, lean, jump, the wheels) go to the player directly and are on time. The replay delays the movement keys by a frame to match. Without that, every state change was one frame early.
- **The jump's height** matches to the centimetre with Strength's push-off bonus; the eye's largest gap is on landing, and the simulation is back in the standing state 0.03 s early (settled by the fourth recording, below: the ground was lower than where the character stood).
- **After a jump** the cap on the selected speed is 0.456 for 0.43 s in both.

**The fourth recording** (2026-10-07, recorder 0.2.0: 28 scenarios run from one spot, the player's own frame time and the body's animator state written on every frame; 25,500 frames in all). The movement state is the same as the game's on every frame of every scenario, and so is the state the body's animator is in or entering. Root mean square and largest gap:

| Scenario | Distance (over) | Eye position |
|---|---|---|
| Walk forward 3 s | 0.1 mm, 0.2 mm (9.3 m) | 0.7 mm, 2 mm |
| Strafe right, back, left, then forward-right | 0.3 mm, 0.7 mm (5.5 m out) | under 0.1 mm |
| Forward, then back | 0.3 mm, 0.5 mm | 0.5 mm, 2 mm |
| Slowest walk | 0.3 mm, 0.5 mm (3.2 m) | 0.4 mm, 2 mm |
| Crouched walk | 2.1 mm, 3.4 mm (4.5 m) | 0.4 mm, 2 mm |
| Sprint | 0.6 mm, 0.8 mm (23.6 m) | 9 mm, 66 mm (the ground) |
| Sprint key held before forward | 0.1 mm, 0.1 mm (10.8 m) | 0.7 mm, 2 mm |
| Standing jump | none | 0.3 mm, 2 mm |
| Running jump | 0.6 mm, 1.1 mm (12.6 m) | 0.5 mm, 3 mm |
| Walk while turning | 0.1 mm, 0.2 mm (13.3 m) | 0.7 mm, 2 mm |
| Walk while aiming | 0.4 mm, 0.8 mm (4.7 m) | 3.8 mm, 22 mm |
| Walk through the stances | 1.8 mm, 2.6 mm (5.4 m) | 0.4 mm, 2 mm |
| Sprint, both keys let go together | 0.5 mm, 0.6 mm (14.2 m) | 2.8 mm, 30 mm |
| Diagonal walks | under 0.1 mm (2.5 m each way) | 0.4 mm, 2 mm |
| Turns of 140 and 84 degrees standing | none | 3.6 mm, 7 mm |
| Lying down and getting up | none | 1.6 mm, 3 mm |
| Crawling forward, back and sideways | 3.0 mm, 4.4 mm (2.8 m crawled) | 1.8 mm, 3 mm |
| Sprint, dive, crawl, get up | 0.5 mm, 0.7 mm (15.4 m) | 1.3 mm, 8 mm |
| Turning the view lying down | none | 2.0 mm, 5 mm |

The velocity figure agrees within 0.004 m/s everywhere but one frame of the reversal and the frame a running jump lands on. What it took, beyond the third recording's corrections:

- **Where the ground is** ("What counts as on the ground" under Gravity). The jump's flight had matched to the millimetre all along; the landing came 4 frames early because the recorded character had been standing 8.0 cm over the ground and landed on it. The replay now puts level ground where the recording's jump found it and uses the game's check distances. Both jumps then land on the game's frame.
- **The fall's limit and when limits take effect** (under Speed). The cap after the running jump lifted 0.39 s early without the fall's share, and both caps began and ended a frame early.
- **The landing's first frame** (under Jumping).
- **The angle a frame old and the update that runs twice** (under Looking). With them the body's bearing through both turns on the spot is the game's on every frame; before, the turn began a frame early and the body was up to 3 degrees off through it.
- **Each scenario's own weight.** A reload that drops a magazine lightens the character, and the recording's last five scenarios show an inertia of 0.050 and then 0.047 where the rest show 0.052. Read once for the whole recording, it made the wind-up before a sprint 0.2% slow and the sprint start a frame late.
- **Prone** is the game's as it was written from the code: nothing in the four states had to be changed. Going down takes 0.90 s from standing and the dive out of a sprint 0.50 s and 1.89 m; getting up 0.83 s; the crawl goes 0.499 m/s forward, 0.401 back and 0.275 sideways, which are the root motion table's figures.

**The fifth recording** (2026-10-08, recorder 0.2.1: the same 28 scenarios, made facing the other way on the same ground, 25,065 frames). It was asked for to settle the weapon in a crawl and settled more than that.

| | Game | Simulation |
|---|---|---|
| Movement state and the body animator's state, all 28 scenarios | | the game's on every frame |
| Distance, upright walks, sprints and crawls | | 0.2 to 2.6 mm (root mean square), 4 mm at worst |
| Distance, the two crouched walks (05, 17) | 4.52 and 5.36 m | 11 and 15 mm out, 22 mm at worst |
| Eye, walks and sprints on the recording's slope | | 0.0 to 0.6 mm |
| Eye, lying down, crawling, diving and turning lying down | | 0.0 to 1.5 mm, 13 mm at worst |
| Height gained crawling 0.52 m to the right with the body rolled | 7 cm | the same |

What it showed:

- **The weapon in a crawl was the hand's hold**, not the turn-away, which the recording has at zero throughout (`weapon-handling.md`, "In the body's hand").
- **The lying body is laid to the ground, and rolled 8 degrees by ground that leans the least bit one way** ("Lying to the ground" above). The fourth recording, facing the other way, had no roll; this one had the eye 20 cm aside from the sixth frame of going down until getting up, and a crawl to the side that climbed.
- **The ground of both recordings slopes** 0.22 degrees, rising 3.8 mm a metre toward the world's -z. With that in the replay (a `ground.txt` beside the traces: the rise per metre along x and z) the eye in the walks and sprints agrees within a millimetre, and the fourth recording's four prone scenarios, which had been 1.3 to 2.0 mm off, come to 0.0 to 1.6.
- **Repeatability.** The two recordings give the same figures wherever the ground lets them: the same states on the same frames, distances within a millimetre or two of each other.

**The sixth recording** (2026-10-08, recorder 0.3.0: the 28 scenarios again on the same ground, facing as the fourth did, and 12 more for aiming, the sidestep and lying down; 38,600 frames). The recorder now writes the ground's normal as the game takes it, the animator's parameters and every layer, and the body's arm bones.

| | Game | Simulation |
|---|---|---|
| Movement state, all 40 scenarios | | the game's on every frame |
| The body animator's state, all 40 scenarios (the reload lying down, 39, since it was built) | | the game's on every frame |
| Distance, walks, sprints and crawls | | 0.1 to 1.3 mm (root mean square), 2.2 mm at worst |
| Distance, the two crouched walks (05, 17) | 4.47 and 5.29 m | 2.6 and 2.3 mm out, 4.1 mm at worst |
| Eye, standing, walking, turning, leaning (the view level) | | 0.1 to 0.7 mm, 2 mm at worst |
| Eye, lying down, crawling, diving and turning lying down | | 0.0 to 1.0 mm, 9 mm at worst |
| The ground's normal | (0, 0.9999929, 0.0037710) on 28,047 frames | the slope fitted from the walks of the fourth and fifth, -0.00377 along z |
| The sprint flag (`Physical.Sprinting`) through an aimed sprint (33) | on at the key, 0.23 s before the sprint state | the same on every frame |
| Sidestep right and left (36): the state, `Step`, `SidestepFloat` | the state on the frame of the key; the value 0.024 or 0.027 a frame | the same on every frame |
| Sidestep: how far aside, and the eye | 0.314 m in 0.35 s; the eye 17 cm lower and 16 cm further out | within 0.1 mm (root mean square), 0.2 mm at worst; the eye within 0.1 mm |
| Sidestep with the sights up (37) | | the same states; distance within 0.2 mm; the eye 0.9 mm, 8 mm at worst as the sights come up |

What the sixth recording added or changed:

- **Three small pieces.** A transition taken at an exit point with no blend hands the rest of the frame to the new state, as one with a blend does (`body-animator.md`); the sprint's flag comes on at the key and takes the sights down then (`firing.md`); and the body leans into the sights (`body-animator.md`).
- **The eye's last millimetres standing are in the view's pitch.** The fourth and fifth recordings were made with the view 6.2 and 5.4 degrees down, and their standing scenarios (lean, the turns) have the eye 2 to 4 mm off. The sixth was made with the view level and has them within 0.2 mm. What is left is therefore in how the layer that bends the body with the view blends its clips away from level, not in the lean or the turn.
- **The roll lying down (38)** was built from it the same day: figures under "Prone".
- **The reload lying down (39)** was built from it the same day: the body's animator is in a reload state of its own for a quarter of the scenario, with no change of movement state and the eye unmoved. Under "Prone".

**The seventh recording** (2026-10-08, recorder 0.4.0: the sixth's 40 scenarios again on the same ground and bearing, with a heavier pack (inertia 0.067 against 0.062), and nine more for what had only been read from code; 47,450 frames). The recorder now also writes what the script asked the view to turn, the movement state's two figures for the view's turning, the weapon's operation in hand and the elbows' goals.

| | Game | Simulation |
|---|---|---|
| Movement state and the body animator's state, all 49 scenarios | | the game's on every frame |
| Distance, walks, sprints and crawls | | 0.0 to 2.0 mm (root mean square), 3.5 mm at worst |
| Distance, the two crouched walks (05, 17) | 4.45 and 5.26 m | 0.8 and 0.2 mm out, 1.4 mm at worst |
| Eye, standing, walking, turning, leaning | | 0.0 to 0.7 mm, 4 mm at worst |
| The first sprint of the character's life (06) | the sprint parameter winds up from 1, not 0.5 | the same: 23.9 m within 2.2 mm |
| The view under each state's limits (11, 12, 22, 28, 44, 45, 46, 49) | | within 0.0005 degrees (under "Looking") |
| The body's angle to the view, a jump with the view turned (45) | | the game's on every frame |
| The prone key in a reload, standing (46) | down at once | the same on every frame |
| A crawl into a reload (47) | the crawl ends, and resumes when the magazine is in | the same on every frame; 1.9 mm |

What the seventh recording corrected:

- **The prone key in a reload.** The refusal read from the code belongs to the online raid's player; the offline one, which every recording is of, has none ("Prone", the command). Scenario 46 was 48% in the wrong state with it.
- **Where the sprint parameter starts.** At 1, until the first sprint ends ("Sprinting"). This recording was begun by a character that had not yet sprinted; with the parameter taken as 0.5 its first sprint ended 27 cm short. The replay now takes the parameter from the recording's first frame.
- **The body's turn in a jump** is the engine's blend of two rotations, not a share of the angle ("Jumping"). No earlier recording turned the view in a jump.
- **An additive layer's curves add to a parameter** (`body-animator.md`, "Layers"). In a jump and getting up from lying, with the view turned 20 to 25 degrees from the body, the game's `Weapon_3rd` read 0.944 where the simulation kept 1 (28, 45, 46, 49). With the rule in, the body's hold on the weapon is the game's within 0.0003 on every frame of all 49 scenarios.

**The eighth recording** (2026-10-09, recorder 0.4.1: the seventh's 49 scenarios again from a spot on the same patch, bearing 9.66 degrees, and seven more, 51 to 57, for what had been built since from the game's code alone; 56,142 frames, every one with the wider record; kept in `local\traces\recording8`). All 56 scenarios replay with the movement state and the body animator's state the game's on every frame. Distances are within 3.2 mm at worst (the crawls), but for the two crouched walks (05 and 17: 5.8 and 7.6 mm at worst over 4.5 and 5.3 m). What its new scenarios settled:

- **A jump out of a sprint** (57) lands in the sprint with the sprint's pace kept and then cut by the jump's own cap ("Where `SprintSpeed` starts"). It corrected one thing: the sprinting state looks at the speed limits a second time itself ("When a limit takes effect"), which the simulation did not, and was a frame late with the cut.
- **The prone key lying in a reload** (56) and **the view standing still through a reload lying down** (55), both as built ("Prone", "Looking").
- **The trigger in a sprint and a crawl** (52) as built (`firing.md`).
- **The sights through a roll** (51) stay up, as built; the eye in it is held off the ground by the camera's own collider, which was not built and now is (`weapon-handling.md`).
- **The character floated 7.6 cm over the ground** on the spot every scenario started from: its standing jump lands that much lower, and the camera's box in the rolls rests that much lower. `ground.txt` carries the figure as a third number, for what meets the terrain.
- **Not settled then:** the weapon pushed back by the ground (53, 54), whose turns stopped short of it. Recorder 0.4.2 rewrote them and the ninth recording has them (below).

**The ninth recording** (2026-10-09, recorder 0.4.2: scenarios 53 and 54 alone, from a spot on the same patch, bearing 17.66 degrees; 4,662 frames; kept in `local\traces\recording9`). Both replay with the movement state and the body animator's state the game's on every frame. What they are for is the weapon pushed back by the ground and the sights with it: `weapon-handling.md`, "Turn-away", has the table. For movement they add two things:

- **The view driven from what the script asked** (`replay --own-view`) follows the game's to 0.0000 degrees through both, lying and crouched with the sights up, down to the view's limits (25.2 degrees lying, 90 crouched), but for one frame's turn at each change of the sights that the obstacle makes (0.06 degrees lying, 0.31 crouched). That is the recording, not the simulation: the recorder writes the turn's multiplier at the end of the frame, after the obstacle has changed the sights, and the game turned the view earlier in that frame with the multiplier as it was.
- **The character rode 7.9 cm over the ground** on this spot (fitted from the weapon's depth, not measured: the recording has no jump). Recorder 0.4.3 writes the figure on every frame (`ground_below`).

**The tenth recording** (2026-10-09, recorder 0.4.4: all 56 scenarios from a new spot, bearing 26.27 degrees, the view 9.54 degrees down when the key was pressed, which no recording since the fifth had; 58,514 frames, every one with the legs' bones and the ground under the feet; kept in `local\traces\recording10`). All 56 scenarios replay with the movement state and the body animator's state the game's on every frame. Distances are within 3.8 mm at worst (a crawl) and most within 1 mm, bar scenario 02, which is the recording's ground. What it added:

- **Why crouched walks came out short in some recordings: the floor's curve is shared** ("Moving"; the first bullet under "What still differs"). The tenth's crouched walk was 39 mm short over 4.4 m before this was found, and is 0.3 mm with it.
- **The legs' bones are the game's** in every gait, to 0.2 mm and 0.1 degrees in the walks (`body-animator.md`, "The body as drawn").
- **The ground under the feet is measured** (`ground_below`: a ray down from the character on the layers the weapon's line is cast on). On this spot the character stood 7.0 mm over the ground; its standing jump (08) lands 6.95 mm lower, and the ray reads 0.003 mm after it. The eighth's spot had 76 mm and the ninth's 79 (fitted). The replay takes the first frame's figure for the terrain where a scenario has no jump of its own.
- **The view pitched down from the start** leaves the eye in a walk and a sprint where the game has it (within 0.05 mm on the patch's own slope). **It shows one thing that is not right, and by how much:** with the body leaning, or the view turned from a standing body, the eye is out by an amount that goes with the view's pitch. Root mean square over the lean (10) and the two turns on the spot (11, 22): 0.1 mm with the view 0.2 degrees down (seventh recording), 0.9 to 1.6 mm at 2.6 degrees (eighth), 1.8 to 3.7 mm at 5.4 and 6.2 degrees (fifth, fourth), 3.0 to 5.5 mm at 9.5 degrees (tenth; 9.7 mm at worst). About 0.6 mm a degree. The legs and pelvis are exact in the same frames and the first spine bone is up to 1.1 degrees out, so it is in how the layers that bend the spine with the view's pitch and turn it with the view's yaw combine. **Not looked into.**
- **Two scenarios carry the recording's own accidents.** In 02 the walk back climbs a bank 7.7 m behind the start (the game's ground normal leans 26 degrees for 21 frames and the character ends 0.35 m up): its distance is 0.83 m out from there on, as it should be on the replay's even slope. In 17 the game's speed wheel came down seven notches between 1.19 and 1.80 s, a tenth of a second apart, which is the game's own pace for the wheel and nothing the script asked: something turned the wheel. The recording's folder holds a script for 17 with those seven notches in it (`scenarios\17-stance-steps.json`), and with it 17 replays within 1.3 mm. Twelve frames of the dive (27) lie on ground with a steep normal, and the sprints past 14 m run onto ground of another slope: the eye is up to 29 mm out there and nowhere else by more than 8.
- **The game's line along the weapon is measured again at the end of each reload** (`weapon-handling.md`, "Turn-away"): eleven reloads, eleven new figures.

What still differs:

- **The crouched walk in the fifth recording: explained by the tenth** (2026-10-09). The game's speed parameter settled at 0.6098 where the fourth recording's settled at 0.6137 with everything in the header the same, and the simulation gave 0.6142 for both; the two crouched walks ended 11 and 15 mm short. It was never the bearing or the slope. A crouched walk's pace is the floor under the walk's speed, and the curve the floor comes from is keyed by whichever character in the raid last had its carried weight change ("The floor's curve is shared"). Read back off the first frame of every recorded walk, the key was that of a character of inertia 0.456 in the fourth and in the sixth recording, 0.909 in the fifth, 0.582 in the eighth, 1.000 in the tenth, and the recorded character's own (0.067) throughout the seventh, with a few walks in each keyed by its own inertia just after it had fired or reloaded. The replay now reads the key off each walk (`floor_keys` in `replay.rs`; a recording has to state the player's frame time for it, so not the third). With it the crouched walks end within 1.4 mm in the fourth, 0.2 mm in the fifth, 1.3 mm in the tenth (which was 39 mm out without), and the worst distance of any scenario comes down from 21.7 mm to 4.0 in the fifth and from 7.6 to 3.2 in the eighth.
- **The fifth recording's running jump** lands on ground 1.2 cm above the even slope; the eye is 4 cm off for the landing's frames.
- **The fourth recording's sprint eye** is up to 6.6 cm off where the recorded player ran over a dip and a kerb (it drops 7 cm, climbs 11 and drops 12 within 2.5 m). The replay's ground is an even slope.
- **The hold changing over.** In the tenth of a second in which a crawl starts or ends the weapon is up to 3.7 degrees from the game's.
- **Not comparable in the third recording:** the player walked into something while aim-walking (scenario 14) and while stepping through the stances (17), and jumped onto ground 1.8 m higher (09) after walking up to it; two more ran over sloping ground (07 and 12, above), which leaves their distances usable and their heights not. In scenario 14 the motion the game asks for, before the obstacle takes its share, is the simulation's on every frame looked at (1.65 m/s, which both count as 1.71, 1.71, 1.52), with the speed setting capped at 0.33 in both.
- **Frame rate.** Several of these rules work per frame, not per second: the standing state keeps 0.8 of its last motion each frame, so in the game a stop runs down twice as fast in time at 120 frames a second as at 60. The replay steps on the recording's frames. The app does not step on the frames it draws (it did for part of 2026-10-08): it runs the simulation on a frame clock of its own at the recordings' 120 frames a second, so the result is the game's at that rate whatever the rate of drawing.
- **A fall off a ledge, in the simulation only** ("Falling"; `a_walk_off_a_ledge_falls_and_lands` in `crates/trp-sim/tests/install.rs`; an untrained character with nothing carried, 120 frames a second). Walking at 2.83 m/s off a ledge 2.5 m high: still in the walking state all the way down; 0.708 s in the air, 1.41 m forward (2.0 m had the pace been kept) and 2.49 m down; the sights due down 0.500 s in, 1.25 m down; the speed setting allowed drops from 0.625 to 0.387 on landing and is back four seconds later. None of it is compared with the game: no recording has a fall. In the app the same from the range's platform, with the sights up when it walks off: up 0.35 s into the fall, down at 0.6 s and after the landing, and the walk held at its capped pace for about 1.3 s.
- **The head's push out of walls** (2026-10-25, 0.132.0): where the capsule is and the limit, on two recordings by hand, to a hundredth of a millimetre standing and 5 mm lying, the limit exactly ("The head's push out of walls"); the push itself not (no walls in the recordings).
- **Speed limits** (2026-10-25, 0.133.0): the armour's on 327 recordings to the sixth place; a hit's from the code ("Speed limits").
- **Room to move** (2026-10-25, 0.131.0): no recording has a refusal; built from the code, `PROVISIONAL` ("Room to move").
- **Slopes and the slide** (2026-10-25, 0.130.0): `courses2\6-slope-steep` and `6-slope-gentle`, rule by rule, every rule to the sixth place or within 0.0006 m/s ("Slopes and sliding"). The slide out of a sprint is from the code only.
- Not looked at yet: diagonals in detail.
