# Mounting and the bipod: the weapon rested on cover, or on its own legs

How a weapon is rested on cover or on its own bipod, and what that changes. Read 2026-10-25 from the decompiled game (`WeaponMounting`, `IdleWeaponMountingState`,
`MovementContext`, `ProceduralWeaponAnimation`, `MountingStrategy`, the recoil, breath and motion effectors,
`Physical`, `FirearmController`), from the install through `probe` (globals.json `MountingSettings`, the weapon
and bipod templates, the body animator, the bipod's prefab) and from recorder 0.13.0's course 8
(`local\traces\courses6\8-blind-mount-*`: the MDR `5dcbd56fdbd3d91b3e5468d5` and the M60E6
`661cec09b2c6356b4d0c7a36`, steps `05-mount` and `06-bipod`, the M60E6's twice) and recorder 0.10.0's
`courses2\4-mount` (the M700, by hand, the only recording of a mount held). The reading before it:
`local\blind_mount_s127.md`, section 2. Our own words. Conventions: Unity's (y up, z ahead); yaw and pitch in
degrees, pitch positive down; a "frame" is a drawn frame.

**What course 8 holds** (checked before relying on it, 2026-10-25): no mount held. Both weapons' first
`05-mount` runs: both presses found nothing (`WeaponMountingComponent.NoPointFound` three frames after each
key). The M60E6's second `05-mount` (`-233241`): both presses found a forward point and entered the mount,
which was left in the same frame (`EnterMountedState`, then `ExitMountedState`, `ExitMountedStateHandler(0)`),
leaving the stance where the mount had put it (0.578). `06-bipod`: the M60E6's bipod deployed lying down at
2.50 s and folded at 12.00 s without a mount (the script never presses `V` lying down); on the MDR, which has
no bipod, both presses did nothing. Since 2026-10-27, `courses8` (by hand, recorder 0.14.0) has side mounts, a
forward one held and fired, and prone bipod mounts held and fired ("Compared with"); still none has a standing
bipod mount or third person.

## The keys

**Two bindings, each a press** (`EGameKey.WeaponMounting`, `ToggleBipods`; `probe keys mount`, `probe keys
bipod`: the install binds `V` and `V` + `LeftControl`, both `Press`, the player's choice of four). Each gives
its command once (`ECommand.WeaponMounting`, `ToggleBipods`). The first goes to `Player.TryMountWeapon`
(`FirearmHandsInputTranslator`), the second to `FirearmController.ToggleBipod`.

**`Ctrl`+`V` ended a mount on the M700, which has no bipod** (`courses2\4-mount`, 18.39 s): read as `V`'s
own binding taken beside the bipod's (the input manager's handling of a combination inside a longer one is
`trp_sim::bindings`'). PROVISIONAL: the commands of that frame were not recorded.

## Who may ask

**`Player.TryMountWeapon`** passes the press on only with a firearm in the hands whose template is
mountable (`IsMountable`: the M700, the MDR, the M60E6 are), no stationary weapon, the character on the
ground, the weapon in no reload, draw, gesture or put-away operation, and blind fire at 0 (`blind-fire.md`,
"What it holds back"). Anything else drops the press silently.

**`WeaponMountingComponent.TryMountWeapon`** then:
- **already mounted**: the mount starts to leave (below, "What ends it"). So the same key enters and leaves;
- **lying down**: the prone search runs only with the bipod out, no step aside and no movement keys held;
  otherwise nothing is found;
- **otherwise**: the standing search runs. Only a player's own character has the component (no bot mounts).

## The search

**Its figures** (globals.json `MountingSettings.PointDetectionSettings`; the class defaults differ and are
not used): edge distance 1.2 m, ray length 1.6 m, the grid between 0.81 and 1.4 m, a vertical grid of 1 m in
20 rays, a horizontal grid of 0.5 m in 15 rays 0.15 m under the top, the second check 0.05 m long in 5 rays
of 0.07 m from 0.05 m above, a wall check of 0.1 m and 0.02 m, a forward point raised 0.02 m and a side
point set out 0.05 m, the steepest top dot 0.707, the steepest lying top 0.939, a side face at most 0.707
from upright, and at most 3 frames waited for a batch of rays.

**The grid's root** is the body's `Weapon_root` (the prefab's `MountingGridRoot` hangs there with no offset:
the M60E6's recorded skeleton, `05-mount-233241`), turned to the look's yaw alone.

**Standing, forward** (`FindHorizontalMountingPoint`):
1. A column of 20 rays straight ahead (the look's yaw, level), 1.6 m long, from the top of a band down: the
   band is the root's height above the feet ± 0.5 m, each end held between 0.81 and 1.4 m above the feet
   raised by the mount's pivot height (`MountingRotationCenter.z`, the bipod's with it out), and the band is
   lengthened by half that pivot height. The rays go down the band in equal steps.
2. **The edge**: of the rays that hit within 1.2 m, the one taken is the first that hit after a ray that
   missed (or hit beyond 1.2 m), the one nearest in height to the root; or one more than a fifth of 1.2 m
   nearer than the ray taken just above it (a step out under an overhang).
3. **Room above it**: a ray from 0.05 m over the edge's hit, straight ahead for 0.05 m, must hit nothing.
4. **The top**: five rays from 0.05 m over the edge's hit, spread 1 cm apart onward, each 0.07 m down. A hit
   at the start of a ray (distance 0) spoils the check; of the rest, the highest above the edge's hit whose
   face is within 45 degrees of level is the top.
5. **Found**: the point is the top raised 0.02 m; its direction is the edge face's normal turned round; the
   side is forward. Without a top, or without an edge, the side search follows.

**Standing, at a side** (`FindVerticalMountingPoint`, `FindHorizontalGridPoint`): a row of 15 rays straight
ahead, 1.6 m long, across 0.5 m centred on the root (from its right to its left), at the root's height held
between 0.81 and 1.4 − 0.15 m above the feet (raised by the pivot height). Among the inner thirteen rays, a
hit within 1.2 m on a face within 45 degrees of upright, whose face turns from the look as much as the most
turned of the row's faces (within 0.001 of dot), beside a ray that missed and has no hits on both its own
sides, is a corner; the nearest to the row's middle wins, the nearer of two equally far. A miss on its left
makes it a `Right` mount (the free edge to the right), on its right a `Left` one. The corner is checked:
nothing in the way just round it (0.1 m), and the face found again from beside (0.02 m in, 0.1 m across). The
point is set out from the corner by 0.05 m along the face's outward diagonal; the direction is the face's
normal turned round. The row is cast twice: on the detailed world layers, then on the terrain's coarse
layer and doors.

**Lying down** (`FindMountingPointProne`): at once, in the frame of the key. The root turned by the look's
yaw and the ground's pitch; at the bipod's reach ahead (the bipod pivot's `y`, unsigned) a sphere of 0.2 m
must be free, and a ray down (the root's own down) of the shorter of 0.4 m and twice the bipod pivot's height
must hit, not at its start, ground within 20 degrees of the root's up. The point is that hit raised 0.02 m
along its normal, forward.

**Frames** (`SetRaycasts`, `RaycastCommand` batches): a batch of rays is answered the frame after it is cast.
So a forward point is found two frames after the key, and a press that finds nothing at the edge's column
and nothing at the sides is answered three frames after it (recorded: `OnFoundPoint` 2 frames after the key in
`courses2\4-mount` and in the M60E6's `05-mount-233241`; `NoPointFound` 3 frames after each key in the MDR's
and the M60E6's first `05-mount`). Only one search runs: a new one cancels the last.

**Found, then** (`OnFoundPoint`): a point on a door being worked is dropped silently; a point on a door or a
moving platform makes that thing end the mount when it changes (`OnStartExitMountedState`; not built: the
range has neither). Then the mount's data is set (below); refused, the data is cleared and nothing happens.
With a canted sight in use, the first straight sight is chosen before entering.

## The mount's data

`WeaponMountingComponent.TrySetPlayerMountingPointData`, with these figures (globals.json
`MountingSettings.MovementSettings`): pitch limits ±15 forward and at a side, ±20 forward with the bipod out;
at most 10 past the pitch limit and 25 past a side's yaw limit at the press; a side's yaw span 23 degrees;
approach 0.2 to 1 s, 1% longer a degree off; exit 0.2 s; the turn clamped to 1 degree a call at sensitivity 1;
camera speed 250; sway with the bipod 0.15.

**The pivot**: the weapon's `MountingPosition`, which is zero on 169 templates (the three here among them);
then the left hand's marker on the weapon plus the moved bone's own place plus (0.05, -0.18, 0.025), in the
`Weapon_root`'s axes (`ProceduralWeaponAnimation.FindMountingPoint`). With the bipod out and a forward mount,
the bipod's: its `bipod_ground` bone in those axes (`FindBipodMountingPoint`; the M60E6's bipod: 0.172 below
its own root).

**Refused** when the look's pitch is more than 10 past the pitch limits; when a side mount's yaw is more than
25 outside its span (taken either way round 360); when the target place cannot be reached (below). Lying down
nothing else is checked.

**The yaw limits**: forward, the look's yaw ± (atan(0.25 / |pivot.y|) − 1) degrees (recorded: ±18.98 on the
M700, so |pivot.y| 0.687 m), the body to face the look; at a side, from the wall's direction to 23 degrees past
it toward the free side, the body to face the wall, the target yaw the end of the span nearer square to the
wall. Lying down, the look's yaw plus the bipod's `YawLimitProneBipod` (±15 on the M60E6's), pitch its
`PitchLimitProneBipod` (-4 to 6), the stance as it is, approach 0.2 s.

**The target place**, standing: back from the point by |pivot.y| along the target yaw and by an offset
across and along that goes from crouched (0.1866, 0.2853) to standing (0.1849, 0.074) with the stance, at the
height of the first static ground within 3 m under the point's height (else the feet's). **Reachable**
(`CheckIfTargetPosReachable`): on the ground, and a box (half √(r/2)/2 across and deep, half (height − step/2)/2
high, r the capsule's radius and skin) from the feet raised by the step height and the capsule's centre, turned
to the target body yaw, swept toward the target for the distance plus r/4, hits nothing static.

**The stance limits**: the point's height above the target place less the pivot's height, as a share of the
grid (0.81 to 1.4 m), is the stance at a level look; each 50 degrees of pitch is a whole unit of stance, so the
pitch limits are cut to what keeps the stance between 0 and 1, and the stance limits are the share less and
more the pitch limits over 50 (recorded: 0.008 to 0.608 with ±15 on the M700, so a share of 0.308).

**The approach's length**: 0.2 s plus 0.8 s times the distance to the target place over 1.2 m (recorded: the
M60E6 0.243 m off took 0.36195 s; ours 0.36196), and with the look's pitch outside the cut limits, longer by
1% for each degree of pitch (from level, forward; from the nearer limit, at a side) and of yaw off the target,
held between 0.2 and 1 s.

## Entering

`MovementContext.EnterMountedState` puts the mount's state over the movement state as an overriding
control state; the movement state in force goes on (`CurrentState` stays standing or walking, recorded),
but every command goes to the mount's (`Player.CurrentManagedState`), and its frame update runs after the
movement state's own (`MovementState.OnStateMove`). The left hand's item is put away. Entering:
- the lean becomes the slow lean's; the lean is put to 0 (its value at the press kept for the approach), the
  step aside to 0, the mounted tilts to 0, the left stance off;
- with a canted sight aimed, the sights go down;
- the view's limits: the turn clamped to 1 degree a call, the pitch limits eased to the mount's (5 a second,
  the movement's own easing, `movement.md`), the rotation function the mount's (below);
- the weapon's motion takes the mount's way (`MountingStrategy`; below), the body's `ProneBipodsOn` is set lying
  down, the hands' animator's `Mounted` (where it has one: none of the M700's, the MDR's or the M60E6's has);
  the ergonomics are worked out again; the weapon's motion told it is mounted and whether at a side;
- **the target pitch**: forward and not lying, level (0); else the look's pitch held to the limits;
- **the stance at once** to where the target pitch puts it between the stance limits (recorded: 0.578 on the
  M60E6's `05-mount-233241`, `pose` on the frame of `EnterMountedState`);
- the enter's sounds (below).

## The approach

For the approach time, every frame (`TryApproachTargetPosition`, the movement's whole-millisecond clock):
- the share t of the time gone;
- the body turned from where it faced to the target yaw (`Quaternion.Lerp`), the look's yaw and pitch from
  where they were toward the targets by t twice over (a lerp of a lerp: about t²), the yaw offset into the
  span by t;
- not lying, the lean from its value at the press toward the entry tilt (a side mount's ±5 by where the yaw
  starts in its span), sent to the camera and the body only at a side;
- the feet moved toward the target place by t along the ground (the place's height left alone), through the
  character's own movement (`ApplyMotion`), with the body's `Inert` up while more than 2.2 cm off. **If the
  character cannot walk** (locked against an obstacle, `movement.md`, "Stopped by what stands in the way";
  or healing its legs) the mount is left at once;
- the turn of the view is ignored while it lasts (`Rotate` returns), and so are the walk's own motion and the
  standing state's rotation (`IgnoreDeltaMovement`);
- at its end, more than 8.9 cm (0.008 m²) from the target leaves the mount at once; else the feet are put
  on the target, `Inert` lowered, and the transition is over. The share can pass 1 (recorded 1.016).

## Mounted

**The turn** (`IdleWeaponMountingState.Rotate`): the frame's turn, at sensitivity 1, is cut to 1 degree
(as a vector) and its yaw part added to the yaw offset, held to the span (forward ±(its half); a `Right` mount
from −span to 0, a `Left` from 0 to span). A turn outward is refused if the head's capsule, carried round the
player's axis by twice the turn and moved sideways by 0.25 m times the offset's share, would be pushed at all
(`RotationOverlapPrediction`; since 0.132.0 the head's own capsule, `movement.md`, "The head's push out of walls";
before it the character's capsule stood in for it, as it still does for settings made without the prefab).
The view's limits are the mount's: yaw between its limits, pitch between the cut limits (the lower one, on a
forward mount, put to the current pitch while the weapon sticks more than 1 cm into something:
`CalculateDynamicDownLimit`), set at once each turn. **The overlap is measured from the hands as placed**
(`WeaponOverlapping`: the hands' place through the moved root), so a weapon on its point runs clear of what it
rests on (recorded: nought on every mounted frame of `courses8`'s window and corners and `courses7`'s sill;
built 0.144.0, `Movement::placed_weapon_overlap`: before it ours measured from the unmoved rig, the sill held
the pitch by this limit, and the view stuttered as the stance followed). A host drawing between frames turns
the view ahead by the mount's turn (`Movement::look_ahead`, `mount_look_ahead`), not the movement state's.

**The stance follows the pitch** (`UpdatePoseLevel`, not lying): between the stance limits by where the pitch
is between the cut pitch limits: looking down raises the body, up lowers it (recorded: 0.008 to 0.47 as the
M700's look went up and down).

**The tilt** (`UpdateCurrentTilt`, not lying): on a forward mount ±5 times the yaw offset's share of the half
span, its sign against the offset's; on a side mount only while the yaw is turned toward the wall (the
offset's sign against the side), and then also the camera's roll and the body's `Tilt`. A forward mount's
goes to the movement's tilt and the networked mounted tilt only: no camera roll, no body lean (recorded on the
M700: `tilt` ±5.00 at the yaw limits, `smoothed_tilt`, the body's `Tilt` and the camera's roll all 0).

**A side mount's drop**: the point's height is lowered by 0.1 m times the offset's share.

**The weapon's place** (`MountingRotationFunction`, the rotation function while mounted): the hands' root
turned to the view as ever, and moved from the ribcage toward the place that puts the pivot on the point
(`WeaponMountingPos`: the root now plus the point, lowered by the side drop, less where the pivot is in the
`Weapon_root`'s axes) by the transition's share (0 to 1 coming in, 1 to 0 going out; 1 held), **held to 0..1**
(`Vector3.Lerp`): the approach's last frame takes the share past 1 (`TransitionProgress` 1.01 recorded at 120
frames a second, 1.0833 at 60) and the exit's last below 0, but the rig stops on its place (recorded, the M700 on
its bipod: the camera on `CameraOffset` from `WeaponMountingPos` to 0.1 mm, where the share unheld would put it
1.4 mm off; built 0.164.1, before it ours carried the rig 8% past at 60 frames a second). **The pivot is
turned by the frame before's turn**: `Weapon_root.TransformPoint` reads the bone as the last frame left it, before
the hands' root is turned to this frame's view, so on a frame that turns the view the pivot comes off the point by
that frame's turn of its reach, and goes back on the next still frame (recorded, below). **The arms go with the
weapon whole**: the shoulders leave the ribcage with it (recorded: the upper arms' reach to the palm and their
distance to the weapon as off a mount while they move up to 0.53 m from the ribcage), so nothing between the
collarbone and the hand is stretched.

**The arms drawn in first person** (recorded 2026-10-27, `courses9\fp-mounted-akm`, recorder 0.15.0; built 0.145.3):
the first-person squeeze along the view (0.664 at a field of view of 74) is put on twice, the weapon's rig about the
rig's own root (`HandsHierarchy.Self`'s scale) and the body, whose arms are drawn, about the ribcage (its scale;
`ApplyFovAdjustments`). Off a mount the rig's root is at the ribcage (`DefaultRotationFunction`) and the two are one;
on a mount the root is moved off it (above), and the arms drawn stand (1 - s) of the root's offset along the view away
from where the rig's squeeze would put them: whole, their shape kept, **the hands going along the weapon**. Recorded:
the hand's offset from its marker along the view = -9.1 mm + 0.995 x that prediction on all 1,012 frames (correlation
1.0000, 0.15 mm rms left over); the game's root 5 to 25 cm behind the ribcage there (its stance high at the press:
the feet 0.756 m back from the point), the hands 19 to 84 mm forward along the weapon. A stance low at the press puts
the feet further back (0.953 m from a full crouch), the root ahead of the ribcage and the hands back along the weapon.
Seen from outside there is no squeeze: the body's hands stay on their markers (recorded, `courses9\outside`: the left
60 mm off its grip's marker on every frame, the right on its own). In the app (`hands_view::squeeze_shift`): the arms'
upper bones and the kept collarbones moved by it in the rig's root's axes; the simulation's figures untouched.

## What ends it, and what it ignores

**Leaving with the 0.2 s exit** (`StartExitingMountedState`): the mount key again; any movement key; a
lean other than to 0 (`IdleWeaponMountingState.SetTilt`): a lean key pressed toward the side the mounted
tilt is not already on (the same side asks for 0 and does nothing: `Player.ToggleLean`'s `SameDir` reads
the mounted tilt), and the slow lean's axis pushed (`SlowLean` asks for the tilt plus a tenth, so anything
but 0) while the lean and the smoothed lean are within a degree (the smoothed lean takes no value while
mounted, so a forward mount turned more than a fifth of its half span from the middle no longer hears it);
the jump (which is then passed to the movement state); blind fire
(passed on); a change of stance (passed on); the step aside (passed on); a reload of any kind, a magazine's
unloading, the chamber's, the fire mode's look, the look all over, the magazine's look and the chamber's
(`FirearmHandsInputTranslator`, `FirearmController`); a door's or moving platform's change under the point.
**The reload's key leaves first, whatever follows** (`Reload`; `QuickReload` only at rest): a magazine found or
none, a magazine of the weapon's own full or not, rounds within reach or none; only a weapon used once
(`IsOneoff`) hears nothing. (Built 0.146.1, `Firearm::reload_key`: before, a magazine filled a round at a time
never left, the M700 lying on its bipod among them; seen in the app, not recorded.)
**Leaving at once** (`ExitMountedState`): lying down or getting up (the prone key, then passed on); a vault or
a climb (passed on); the approach's own failures (above); the stock folded; the inventory opening; the
backpack dropped; any change of the thing in hand (`HandsChangingEvent`). **Ignored**: the sprint, the
speed's change and the walk key, a lean key let go, the movement keys' release. **Refused while mounted**: the bipod key, the left
stance (not built), the hands' gestures. **Allowed**: the sights (not a canted sight: the mount picked a
straight one), holding the breath, firing, the tactical devices.

**The exit** (`StartExiting`): once only. The weapon's motion told it is no longer mounted at its start, the
hands' animator's `Mounted` down; the transition's share from 1 to 0 over 0.2 s (the movement's clock);
a side mount leaves the body's `Tilt` at the mounted tilt; the exit's first sound; input is still swallowed
(the turn ignored; the commands the mount's state has none for are dropped). When 0.2 s have passed the state
is taken off (`ExitMountedState`): a forward mount's tilt to 0, a side mount's left as the lean (up to ±5);
the mounted tilts to 0, the yaw limits off, the pitch limits eased back to the stance's, the default rotation
function, the weapon's usual motion, the ergonomics again, the body's `ProneBipodsOn` down, the second
sound. **The stance is not given back**: the character stays where the mount left it (recorded: 0.578 after
the M60E6's immediate exit). Recorded on the M700: both exits 0.200 s from the key.

## The weapon's motion while mounted

`MountingStrategy` in place of the first person's:
- **the fixed steps**: the walk's bob and the hand shake do not run; the turn-away's depth is not zeroed by
  the pose blends; the rest as ever;
- **the pivots**: the weapon's turn (its rotation spring, the sway) and the recoil's turn go about the mount
  pivot (the bipod's with it out on a forward mount) in place of the rotation centre and the recoil pivot;
  the tactical reload's stiffening too;
- **the turn-away**: its value held at 1 at most (no turn-away pose blend), its rotation skipped
  (`AvoidObstacles`), its pull-back kept;
- **the camera** (`MountedWeaponCamera`, after the bones in place of the first person's camera pass): the
  rotation spring's rest is the lean alone (no blind fire, no pose blends' camera turns); the place's target
  worked as the first person's (aimed: the sight's line; the turn-away's pull not taken off), reached across
  and up at `dt × 250` (at once) and along at the aimed or unaimed easing times 250; the rotation as the first
  person's; the hands' recoil turn added; the camera's recoil;
- **the tactical reload's blend** runs to 1 (no delay, 4 a second) while mounted forward with the bipod out,
  as for an aimed reload, and back (0.2 s, 2 a second) when either ends (`UpdateTacticalReload`). It holds the
  weapon bone toward its rest (`TacticalReloadStiffnes`, `TacticalReloadFixation`: `firing.md`, "The reload with
  the sights up"), its turn about y put back about the bipod's pivot's depth, so a bolt is worked with the weapon
  held nearly still. The mount's camera does not read the blend: it takes the clip's turn whole
  and follows the sight where the held bone carries it (`MountedWeaponCamera`; read 2026-10-31, correcting this
  line's "takes the camera's animated turn away", which is the first person's `LerpCamera`).

**Recoil** (`NewRotationRecoilProcess`): each shot's push on the hands' rotation is multiplied by the
template's `MountVerticalRecoilMultiplier` at a side (0.85 on the three), the bipod's `BipodRecoilMultiplier`
forward with it out (0.2 on the M60E6's), else `MountHorizontalRecoilMultiplier` (0.6). The return speed by
`MountReturnSpeedHandMultiplier` (1) or the bipod's `BipodReturnHandSpeedMultiplier` (1). The camera's snap
toward the weapon (`CameraSmoothRecoil`) by `MountCameraSnapMultiplier` (3), or the bipod's
`BipodCameraSnapMultiplier` (1) forward with it out.

**Breath** (`BreathEffector`): no hip inaccuracy is added by a shot while mounted; the breath does not run at
all while mounted unless the arms' stamina is at its floor or an arm is broken, and then at the template's
`MountingVerticalOutOfBreathMultiplier` (0.8) at a side, the bipod's `BipodOutOfStaminaBreathMultiplier` (0.1)
forward with it out, else `MountingHorizontalOutOfBreathMultiplier` (0.4). What the breath last put in the
recoil's offset (the hip wander) is not held, though: **every `SetMountingData`** (entering, starting to leave,
leaving) works the weapon's figures out again (`UpdateWeaponVariables`, `Shootingg.OnWeaponParametersChanged`,
`RecalculateRecoilParamsOnChangeWeapon`), which ends in `HandRotationRecoil.Reset()`: its place, speed and offset
to nought. Recorded on the M700 (`local\traces\m700-bipod`): the weapon turned 0.69 degrees in x lying, the hands'
spring's 0.20 alone on the frame of the mount. Built 0.164.1 (`Hands::set_mounting`); before it ours froze the
wander, 0.427 degrees about the pivot. The same reset follows the weapon's other recalculations (taken in hand, the
stock folded, the weapon modified, an arm damaged, the armour's ergonomics): read, not built.

**Sway** (`MotionEffector`): mounted forward with the bipod out, the movement and the mouse's sway at 0.15
(`MountingSwayFactorModifier`).

**The arms' stamina** (`Physical`): aiming while mounted drains at globals' `MountingVerticalAimDrainRateMultiplier`
0.85 at a side, `BipodAimDrainRateMultiplier` 0.2 forward with the bipod out, else
`MountingHorizontalAimDrainRateMultiplier` 0.6.

**Ergonomics** (`TotalErgonomicsFunc`): while mounted the weapon class skill's level times 0.2
(`MountingErgonomicsGainPerLevel`), or 0.5 forward with the bipod out (`BipodErgonomicsGainPerLevel`), is added
to the share the ergonomics is multiplied by. PROVISIONAL: read at face value (at level 10, +2); no recording
has the mounted total.

## The bipod

**Who says yes** (`FirearmController.ToggleBipod`): the weapon has a bipod (a part in a bipod slot); the
gestures' queue takes one; not mounted; the weapon not stuck into anything; not sprinting; the body free to
make a gesture; the hands' animator idling; and the operation in hand idling (only `Idling` starts it).

**The toggle** (`ToggleBipodOperation`): at once the bipod is out (or in), the hands' animator's inventory
flag down and its `Bipod` set (where it has one), blind fire put to 0, the body's gesture asked
(`BipodForwardOn`/`Off` for a bipod that folds forward, the Harris and Atlas kinds; `BipodBackwardOn`/`Off`
for the rest: the M60's, the PKM's, the RPD's, the SV-98's, the G36's; `BipodViewController.BipodToggleDirection`),
and the weapon's motion told the bipod is used (`IsBipodUsed`, which the recoil, breath and sway read only
mounted). The operation ends on the body's `OnBipodToggle` event, or fast-forwarded, back to idling.

**The body's gestures** (`probe bodystates bipod`, `InteractionsLayer`): `31 Bipod Forward On` (event at
0.5, out at 0.73 over 0.3 of the state), `32 Bipod Forward Off` (0.6; 0.7, 0.3 s), `33 Bipod Backward On`
(0.5; 0.75, 0.25 s), `34 Bipod Backward Off` (0.6; 0.75, 0.25 s), all at speed 0.9, entered over 0.25 s.
Recorded on the M60E6 (`06-bipod`): `InteractionIndex` 33 on the press's frame, the layer's way in over 0.25 s
from the next frame, the operation back to idling 0.567 s after the press, the way out 0.84 s after it.

**Lying down with the bipod out and mounted**: the body's `ProneBipodsOn` sends the aiming layer's
`Aiming_Prone` to `Prone_Bipods` (`Prone & ProneBipodsOn & !InventoryOperation`, a blend of a tenth of the state);
the layer `Bipods` has weight 0. The state blends nine clips of 0.13 s, held at their ends, by `DeltaRotation`
(-25 to 25) and `Aim_angle` (-4 to 6); each ramps `Hand_Left`, `Elbow_Left`, `Shoulder_Left` and
`First_Person_Curve_Weight` from 0 to 1 (and the middle column `Weapon_3rd` too), which on the additive layer adds 1 to
each (`Weapon_3rd` comes to 2). So **the left arm is the body's**: the hand off the weapon, down on the ground under the
stock (recorded, `local\traces\m700-bipod`: `left_hand_first_person` 0 on every frame of six mounts, the curves at 1
0.13 s after the mount's frame, the left palm 28 cm left of the right and 11 cm below it). **The weapon is not handed
to the body's weapon bone on a mount**, whatever the curves (`ThirdPersonWeaponRootAuthority` nought in
`IsInMountedState`), and **the shoulders are held on the rig only mounted upright** (`SetShoulders(1, 1)` mounted
and not lying; lying, the clips' shoulder curves as ever). Built 0.146.0 (the state among those the body reads; the
mount's two rules in `body`). Compared at the mount's middle and 6 degrees down (the game's 12.36 to 13.04 s):
the body's weapon bone and ribcage the game's to 0.1 mm, the first-person weapon in the camera's axes within 6 mm.
**The camera's place on this mount** (open from 0.146.0 to 0.164.1, 7.8 cm right of and 6.6 cm above the game's):
the gap was the comparison's (the press scripted with the body on the look, on a floor at the feet), and the
rig's share and the hip wander above; with the game's conditions the place is the game's ("Compared with").

**Its legs**: moved by the bipod's own animator (an `Animator` on its prefab's root, with a `StreamingAnimator`,
`BipodViewController`, `BipodAudioController`; read 2026-10-27, `probe partstates`): one flag `Bipod`;
`BIPOD_IDLE_CLOSED` (0.333 s) to `BIPOD_OPEN` (1 s) when it is set, on to `BIPOD_IDLE_OPEN`, and back through
`BIPOD_CLOSE` (1 s); no blend. The open and close clips carry the marks `BipodOpen` and `BipodClose` (the Atlas V8's at
0.237 and 0.254 of its clips, the M60E6's at 0.265 and 0.358), which its sound player plays. The game runs it as one
with the weapon's animator (`WeaponPrefab.UpdateAnimatorHierarchy`): `FirearmsAnimator.SetBipod` at the toggle reaches
it whether or not the hands have the parameter. Recorded: the Atlas's unfold 0.242 s after the press and its fold
0.262 s after (`local\traces\m700-bipod`), the M60E6's unfold 0.266 s after (`courses8`). Built 0.145.0
(`WeaponAnimator`'s parts: the part's own controller on the bones under it, its pose over theirs, its marks with the
weapon's).

**The `_procedural` nodes** (read 0.224.1; `probe partbinds <template>`, `probe tree <bundle>`): six of the seven
bipods the database fits hang each moving joint as a pair, a parent named `<joint>_procedural` that holds the joint's
rest place and turn, and under it, unturned, its one child `<joint>`, which holds the meshes and the next pair (the
Atlas V8's six: the vertical and horizontal axes, both side axes and both legs; the SV-98's four, the G36's three, the
PK's and the RPD's four each, the M60's two; the Harris S-BRM has none, its clips turning its axes directly).
**Nothing moves the parents.** Every curve of the 28 clips of the bipods' own animators binds a node of the prefab, and
not one a `_procedural` one: the clips move the children and the buttons. The parents carry no script and no
constraint, and the game's code names one bipod node only, `bipod_ground`. Of the hands' clips, only the RPD's bind
its bipod's nodes, every one, each held in all 67 clips at the prefab's rest (the clips leave the bipod's root unturned, where the weapon
hangs it a quarter turn about x, and its first node unturned, where the prefab turns it a quarter back: under the
slot, the same place and turn). The PKM's, M60E4's, SV-98's, the G36's with
its bipod and the M700's with the Atlas bind none (the PKP, PKTM, M60E6 and the Atlas's other hosts not read). So,
folded or out, mounted or not, the parents stand at the prefab's rest. Their names suggest code that moves them; none
is in this build, and why they are so named is not read. TRP does the same: the part's animator takes every node
under the part as a bone at its prefab rest, and a bone no clip moves starts each update from it.

**When the weapon changes** (`WeaponModified`): the bipod stays out only if there still is one.

## Sounds

- **Entering**: the kit's medium rattle (`Player.EnterMountedStateHandler`: `GearMedium` at its bank's random
  volume); after the approach time, the weapon's `VerticalMountSounds` at the weapon (`WeaponSoundPlayer`, for
  any mount, cancelled by an exit); and on a forward mount, after the approach time, the bipod's surface sound
  (`BipodAudioController`; on weapons whose bipod has one).
- **Leaving**: the kit's medium rattle at the exit's start and again at its end (both calls of the exit
  handler). Recorded on the M700: `weap_vertical_mount_04` 0.459 s after the mount (approach 0.451) and
  `_02` 0.508 s after it (approach 0.507); `gear_medium` at each mount, each exit's start and its end. On the
  M60E6's immediate exit: two `gear_medium` clips on the one frame.
- **The bipod's open and close** (`BipodAudioController.OnBipodOpen`, `OnBipodClose`) come from the marks of the
  bipod's own animator ("The bipod", its legs), not the hands'. (Read wrongly at 0.125.0 as silent on the M60E6:
  `courses6`'s `06-bipod` held no source; `courses8` has `bipod_rpd_unfold_1` at 0.6, 0.266 s after the press.)
- **The bipod's own player** (`BipodAudioController`, on the bipod's prefab; read 2026-10-27 with `probe bipodaudio`):
  subscribed for as long as the part is fitted, out or folded. A forward mount starts a timer of the approach's
  time (an exit stops it); at its end a line from the bipod toward the mount's point, 1 m at most, on the footsteps'
  layers, gives the surface (concrete when it meets nothing), and that surface's set plays at the bipod: one clip at
  random, at the set's volume, heard to its farthest (`PlayAtPoint`; stereo for the character's own eyes). The M60E6's
  bipod: every surface the same five `bipod_stand_on_*` clips at 0.06, 20 m; open three clips at 0.6, close three at
  0.75, 25 m. Recorded: `bipod_stand_on_1` at 0.06 with the weapon's mount sound on one frame (`courses8`, 14.269 s).

## Third person

The mount's packet (`MountingPacket`) carries Enter (point, direction, approach time), Update (the side
drop), StartLeaving and Exit; another's view lerps the weapon by time, sets `ProneBipodsOn`, and only a side
mount rolls the camera. Seen from outside the character keeps its stance, tilt and body turn from the same
movement; the weapon hangs from the third-person bone by the same rule. Built by choice from the code; read from code, not recorded.

## The battle screen's icon (`BattleUiBipodPanel`, `BattleStancePanel._bipodPanel`, `BattleUIScreen`; read and built 2026-10-25)

The game shows a mounting icon. It is the stance panel's (`MountStance` under `BattleStancePanel`, 100 by 100 at (185, 130) of the panel, `probe scenepanel level44 "BattleUIScreen Variant/BattleStancePanel"`, `local\stancepanel_s133.txt`): `Mount` (`Icon_UI_Bipods`, 30 by 35, off in the scene), `UnavailableMount` (`Icon_UI_Bipods_Red`, the same place, on at alpha 0) and `MountDirections` with three arrows (`ForwardMount` under the icon, `LeftMount` and `RightMount` beside it, each off). Its script (`probe battle`, `local\mount_icon_probe_s133.txt`) pairs `ForwardMount` with the side 0, `LeftMount` with 1 and `RightMount` with -1 (the scene's pairing, kept as read: the game's `Left` is -1), and its flash lasts `_highlightDuration` 0.8 s.

**Shown** (`ChangeMountImage`), as the panel is shown and whenever the mount's data is set or cleared (`PlayerMountingPointData.SetData` at the mount, `ClearData` at the exit's end: `OnMountedStateChanged`): a flash under way stopped (its red image back to alpha 0), every arrow off, the icon on while mounted (`IsInMountedState`: the data held) and then the arrow whose side is the mount's. **Flashed** (`OnMountUnavailable`), whenever the mount finds nothing (`WeaponMountingComponent.NoPointFound`, `OnNothingFound`: the standing search answered with nothing, lying down without the bipod out or with a step aside or a walk key, or a point found and its data refused, which clears the data first), unless a flash is under way: the red image faded to whole in half the length and out in the other half (`DOFade`, DOTween's default ease). The battle screen raises the stance panel at both (`PullUpBattleStancePanel` on `OnMountedStateChanged` and `OnNothingFound`). A door being worked under the point gives neither.

**Built** (`trp_data::screen::MountIcon`, `battle_view`, `Controller::mount_refusals`, with `TARKOV UI`): the icon and the mount's side's arrow while `Movement::mounted`, the flash on every `MountEvent::Refused`, the panel raised at both. Looked at: `local\shots\mount_icon_s133.png` (crouched at the window, mounted forward: the icon right of the figure, the forward arrow under it) and `mount_icon_red_s133.png` (standing there, refused: the red icon fading). Not compared with a recording (the recorder had not written the panel; 0.14.0 does).

## Built

(2026-10-25, 0.125.0.)
- **The data**: `trp_data::weapon::BipodPart` on `Weapon::bipod` (the first bipod of the build in the game's
  walk of the slots, its `bipod_ground` bone, its fold's way from its prefab's `BipodViewController`);
  `trp_data::sound::mount_sounds` (`VerticalMountSounds`); `BreathData::lower_limit` (`EnergyLowerLimit`).
- **The search and the data** (`trp_sim::mounting`): `Settings` from globals.json; `Search::standing` (the
  column, the room above, the top, then the side row on two layers; answered on the frame its batches give:
  2 found ahead, 3 nothing), `search_prone`; `data_for` (the refusals, the limits, the target place, its
  reach by a swept box, the stance limits, the approach); `WeaponMount` (the template's figures, the pivot
  from the left hand's marker where `MountingPosition` is nought, the bipod's); `MountEvent`, `HandsMount`,
  `MountedMotion`, `drain`; `tell` (after the movement's frame: the weapon's `Mounted`, the weapon motion's
  mount and way, the skill's ergonomics gain, a canted sight swapped or the sights put down).
- **The world**: `World::ray`, `sphere_clear`, `box_sweep`, `capsule_push` (the head's), `turn_blocked` (without the prefab's head capsule; the ground alone unless a host
  says); `collision::Block::sphere_overlaps`, `upright_box_overlaps`; the range's (its solids and doors).
- **The mount's state** (`trp_sim::movement`, `movement/mount.rs`): the key (`Input::mount`, `mount_may`),
  the search answered before the animator, `enter_mount` (the slow lean, the lean and the step to 0, the
  target yaw and tilt, the pitch limits, the stance at once), the approach (the look by a lerp of a lerp,
  the body turned, the feet by the frame's last motion, the walk's lock leaving at once), held (the yaw
  offset clamped, the dynamic lower limit, the stance by the pitch, the tilt by the yaw, a side's drop),
  the exit (0.2 s, then the state off; the stance kept); every command routed as the game's mounted state
  takes it; the root motion ignored and the body not turned; the smoothed lean frozen and the weapon's lean
  the mount's camera lean; the body's `ProneBipodsOn` and its `Tilt` at a side; the arms' drain; the
  inventory, the backpack, lying down and a vault leaving at once; gestures refused. `mounted_rig_apart`,
  `mount_view`, `mount_figures`, `weapon_tilt`, `leave_mount`, `set_blocked`, `bipod_gesture`, `gesture_free`.
- **The weapon's motion** (`trp_sim::hands`): `set_mounting` (the pivot for the turn and the recoil's turn,
  the turn-away held to 1 and not turned, the recoil's multipliers and return, the camera's snap, the breath
  stopped or by the mount's share, no hip spread from a shot, the sway on the bipod), `set_mount_strategy`
  (no walk's bob, the camera's rest the lean alone, the mounted camera: the offset from the moved rig blended
  to the sight's eye by the aiming weight at 250×, the bones' turn whole), `set_bipod`, `set_rig_shift`,
  `set_mount_ergonomics`, `straighten_for_mount`; `recoil::RotationProcess::set_mounted`.
- **The weapon** (`trp_sim::firearm`): `may_mount`, `set_mounted` (the animator's `Mounted` where it has one,
  the tactical blend forward on the bipod), `toggle_bipod` (its refusals; `Event::Bipod`), `bipod_toggled`,
  `bipod`, `toggling_bipod`; `Event::LeaveMount` from a reload, the looks, the fire mode's look (with the
  exit), the fold and the put-away (at once). The body's four bipod gestures (`Interaction::bipod`, game
  numbers 18 to 21, animator 31 to 34) and their mark (`Acted::BipodToggled`).
- **The stamina, the skills, the sounds**: `Stamina::set_aim_drain_by`; `WeaponBuffs::mount_ergonomics`;
  `StepSounds::kit_medium` (`GearMedium` at its bank's random volume).
- **The switch**: `trp_sim::Systems::mounting`; the options' `MOUNTING` under `SIMPLE`, after `BLIND FIRE`.
- **The app** (`controller`, `hands_view`, `hands_sound`): `WeaponMounting` (`V`) and `ToggleBipods`
  (`Ctrl`+`V`), a script's `cmd:WeaponMounting` and `cmd:ToggleBipods`; the rig moved onto the point in
  either view (`Shown::rig_apart`), the camera's object left at the ribcage; since 0.138.1 the rig's place worked
  out again for the view as drawn and the mount's camera moved with it (`Controller::rig_apart_drawn`,
  `Sprung::shift`: only the drawing), and in first person the kept collarbones carried with the rig
  (`body_view::see_collarbones`); the kit's rattle at the mount,
  the exit's start and its end, the weapon's mount sound the approach's time after the mount (the frame of
  the mount not counted), at the aiming sound's volume; the sight key passing canted sights on a mount.
  Third person plays the same body states and the moved rig (by choice, from the code).
- **The replay** (`bin/replay`): the two `cmd:` presses, the hand-over, the bipod's gesture and mark, the
  hands' rest flag, `mount`, `mount_target`, `mount_point`, `mount_limits`, `mount_approach`,
  `mount_transition`, `mount_figures`, `tilt`, `p_Tilt`, `p_ProneBipodsOn`, `interaction_shown`,
  `bipod_operation`, `rig_apart`; a folder's `mount.txt` (`sill`, `floor`, `blocked`, and since 0.164.1
  `terrain <depth>`: the terrain under the feet for a script with no trace of its own) for a world the
  recorder does not keep; the rig's bones in the body's axes with the mount's move of the rig.
  **`bin/mount`**: a recording's mounts by hand beside the rules.
- **Tests**: `mounting::tests` (four), `tests/sim/mounting.rs` (five; the held pivot on its point since 0.164.1),
  `tests/sim/hands.rs`'s `a_mount_drops_the_hip_wander`, `range::tests::the_window_sill_takes_a_mount_crouched`.

## Compared with

**The M700 lying on its Atlas, `local\traces\m700-bipod`** (recorder 0.14.0, by hand): the first mount
(12.15 to 28.87 s) replayed by script with the game's conditions at the press: the look 8.13 degrees right of the
body (which lying down had stayed at 18.21 while the look went to 26.34), 13.04 down, the ground 6.71 cm under the
feet (`ground_below`: the body rides over it), the recorded build, a field of view of 74 (`local\scenarios_place_s165`,
`local\traces\m700_place_s165`; `local\place_cmp_s165.pl` sets every held game frame beside ours). **The point** at
the press the game's to 0.1 mm (0.740 m along the look from the body's `Weapon_Root_Third`); the approach turns the
body onto the look, the point staying put. **Held** at the mount's middle (26.34, 6.00), 90 frames: `camera_pos` and
the eye within 0.06 mm; the weapon in the camera's axes, the right palm, upper arm and forearm within 1.5 mm, 0.5
on average; the left palm (the body's) and the ribcage to 0.01 mm. Turned unaimed, (16.63, -4.00) and (19.34,
0.03): within 1.5 mm at most; (14.87, -3.83), 194 frames: within 0.6 mm. Aimed through the scope at 35 degrees,
(15.75, -1.38) and (13.53, -4.00), 51 and 139 frames: within 0.9 mm on average, 2.7 at most while the game's camera
eases in. **The 6.7 cm** is the character riding over that road within its ground check, standing as lying (the
recorder's rays down meet only the road's collider there; `movement.md`'s ride over ground that falls away, built):
with the feet on the app's level floor the same press is refused, as the rule gives (the root 0.1988 m over it).
**In the regression** since 0.164.2 (`r21`: `19-`, `22-` and `23-` with `held.txt`, the replay setting the six held
moments above beside the game's frames, gaps in mm). **After the aim** the game's camera seemed to hold one place
through every look: the recording is in the third person's view from 22.703 s to 50.766 s,
where the game moves no first-person camera, as the replay does from outside. On a mount
in the first person the camera rides the moved rig after an aim and a shot too (`courses2\4-mount`: 12.7 and 11.4 mm
of camera a degree of look), as built.

**Recorder 0.14.0's `courses8`** (by hand, 2026-10-27): `mount-on-window` (the AKM, one forward mount
held 22 s, four aimed bursts), `mount-on-corners` (the AKM, four side mounts at a wall's corner, three left and one
right), `m60-mounting-with-bipod` (the M60E6 with its bipod out from the start: three standing presses, 30 searches lying,
three prone bipod mounts, a 19-round burst and two short ones on it), `pressing-v-on-open-ground` (the AKM: 43
presses standing, in a sprint, a jump, a reload and lying down). `bin/mount` (now taking side and lying mounts as
`movement/mount.rs` does; `local\mount_c8_*_s142.txt`):
- **All eight mounts' data** as the game's: the target within 0.1 mm, the look's target yaw, the yaw, pitch and
  stance limits, the approach (0.28889, 0.39950, 0.34087, 0.31752, 0.39047 s; lying 0.2 s), the target the feet
  lying down. **The entry**: the target yaw (a side's the look's own where it is in the span), the target pitch
  (the look's held to the movement's limits), the yaw shift, the entry lean (-1.8662, -4.2280, -1.2800, 0) and the
  stance at the start (0.8329, 0.2202, 0.8422, 0.9827 at the corners) gap 0.0000.
- **The approach** on every frame: the share, the look, the lean toward the entry tilt and a side's drop gap
  0.0000; the movement's pitch limits easing to the mount's at 5 a second of the player's clock gap 0.0000 (standing
  first on the frame after the mount's, lying on its own: the order ours has), holding the look as they close in
  lying down (from 25 to the bipod's 6 degrees).
- **Held** (2,511, 189, 350, 600, 363, 2, 820, 1,229 frames): the stance by the pitch within 0.00001, the lean by
  the yaw within 0.0001 (a side's only toward the wall, -5 at its end), a side's drop (-0.1 m at the span's end)
  0.0000; lying down nothing moved, as the rule says. The camera's tilt is the lean on every frame at a side; the
  weapon's lean is the frame before's (ours by the same order).
- **The exits** 0.2001 to 0.2070 s: by the key (the window, the three on the bipod), by a stance key (the corners,
  3.600 and 9.165 s: the stance set to 1 on the frame), by a held walk key (19.103 and 25.796 s: the exit asked on
  every frame, the walk taken up on the frame after the state is off); a side mount's lean left as the lean (-5,
  0.037). The sounds: the kit's rattle and the weapon's mount sound on our frames.
- **The search**: found 2 frames after the key (five) and once 3 (a side point, the corners at 13.740 s: a top cast
  first or the coarse layer, which only the corner's geometry can say), nothing 3 frames after (37); a point found
  and refused by the data once (the corners at 21.675 s: the point is not recorded on a refusal, not checked).
  **Refusals** as ours: in the air, in a reload (the key goes no further than the player), lying down without the
  bipod (nothing found on the key's frame); a sprint does not refuse the search. **Lying with the bipod out** the
  point was found only where the body's `Weapon_root` stood over 0.2 m above the ground, the sphere's radius
  (found at 0.204, 0.217, 0.225 m, looking 12 to 24 degrees down; 27 refused, at 0.198 m at most): ours
  the same rule, refusing at a level look and mounting at 21 degrees down on the replay's level floor. The game's
  feet lay 1.6 cm over the ground after a crawl and ours on it, so on level ground ours
  needs about 2 degrees more of a look down.
- **The camera's snap** (`camera_smooth`): 21 at the forward mount's shots against 7 unmounted (×3), 7 on the
  bipod (×1).
- **Recoil on the bipod**, the recorded build fired by a script on the replay's floor (60 frames a second against
  the game's 120; `local\scenarios_bipod_s142`): single aimed shots climbing 0.119 to 0.162 degrees against the
  game's 0.110 to 0.173, the camera 0.358 to 0.452 against 0.354 to 0.448, back 18 to 25 mm against 17 to 25; a
  19-round burst building to the same plateau, rounds 6 to 19 on average 0.570 degrees against 0.586, back 47.3 mm
  against 51.4, the camera 1.367 degrees against 1.448. **On the window mount** (forward, the AKM, its build, on a
  `mount.txt` sill): ours exactly 0.60 of ours unmounted round by round (the template's
  `MountHorizontalRecoilMultiplier`); the game's climb over ours by 9% on the first round and 17% on the fifth
  (0.69 to 0.88 against 0.63 to 0.76 degrees; 3.45 and 3.51 against 2.74 to 3.03), its pushes back within ours'
  spread. **Open**: four bursts against three, and the game's unmounted baseline only from another recording
  (recording12's stock AKM, 60 frames a second: the mounted climb 0.64 to 0.72 of it). One take with the same
  weapon fired aimed mounted and not would settle it.
- **Not in these**: a standing bipod mount (the M60E6's three standing presses found nothing), third person, the
  breath and sway on a mount (no recording held the breath or ran the arms down mounted), the skill's ergonomics
  gain (the profile's class skills are at 0: `TotalErgonomics` never moved, as the rule gives), the bipod's sound
  on the ground (recorded, `bipod_stand_on_1` at the approach's end; not built).

**Recorder 0.14.0's `courses7\10-anything`** (the M4A1 on a window sill by hand, 2026-10-25; three takes with
mounts, `anything`, `-212814`, `-212842`):
- **The data, with `bin/mount`** (`local\mount_c7_*_s137.txt`): on all twelve mounts the target within 0.2 mm, the
  look's target yaw within 0.015 degrees, the yaw, pitch and stance limits, the approach (0.33216 s against 0.33216),
  the stance at the start; the approach's share and look (0.0000 but one mount's 0.26 degree in pitch); held, the
  stance by the pitch within 0.00001 and the lean by the yaw within 0.0001 on 694 to 2,006 frames; the exits 0.2000
  s (two at 0.1653 and 0.2065: the clock's frames); the mount sound on the game's frame.
- **The pivot's turn** (`local\mount_pivot_s137.pl`: the recorded `weapon_root_anim` bone turning our pivot, beside
  the recorded point): off the point by 10.3 and 9.9 mm a degree of the frame's turn (`-212814`, `-212842`; 401 held
  frames each, 1.0 and 0.3 mm rms left over), the pivot's 0.611 m lever giving 10.7; back at once when the turn stops,
  in pitch as in yaw. Built so since 0.138.1 (it was this frame's turn, `PROVISIONAL`).
- **The arms** (`local\arm_len_s137.pl`): each arm's upper arm, forearm and reach to the palm the same mounted and
  not (left 0.254, 0.234, 0.280; right 0.288, 0.224, 0.195) while the upper arms leave the ribcage by up to 0.53 m and
  keep 0.255 m to the weapon. Ours: the rig's arms held their lengths (0.241, 0.229, 0.305; 0.288, 0.224, 0.193:
  our own character's) but the collarbone the sleeves hang on stayed on the body, 0.11 to 0.28 m from its upper arm;
  0.11 m throughout since 0.138.1.
- **The weapon against the view** (`local\mount_incam_s137.pl`, the recorded eye and camera): the game's weapon moves
  less than 1 mm a frame in the camera's axes while the view turns at the 1 degree cap. Ours, in the app on an uneven
  scripted turn (`local\mouse_args_s137.txt`), 1.47 mm a frame rms and 4.7 at worst before 0.138.1, 0.02 and 0.10
  after (`local\mount_frames_rand_*_s137.txt`).

**Course 8 of `courses6`** (recorder 0.13.0; the MDR and the M60E6, `05-mount`, `06-bipod`, the M60E6's second
`05-mount`), replayed from the course's scripts with the recorded builds (`local\mount_run_s132.sh`; reports
`local\mount_replay_*_d_s132.txt`, the side-by-sides with `local\blind_side_s130.pl`):
- **What it holds** (checked before relying on it): no mount held. The first runs' four presses found
  nothing; the M60E6's second run found a forward point twice and left the mount in its first frame; `06-bipod`
  put the M60E6's bipod out lying down and folded it, unmounted; the MDR has no bipod.
- **The search's frames**: nothing found 3 frames after each key, all four presses (the sim's at 1.031 and
  10.029 s on the MDR, the game's `NoPointFound` at 1.0314 and 10.0292 s; the M60E6's 1.030 and 10.030 s against
  1.0304 and 10.0300 s); on the second run, found 2 frames after each key (1.022 and 10.023 s against 1.0216 and
  10.0227 s), entered, and left in the same frame both times, as the game did, with the walk's lock on at the
  start (`mount.txt`'s `blocked`: inferred, the lock is not recorded).
- **The data on the second run's first mount**, on the sill `mount.txt` sets down from what the game recorded:
  the target place within 0.1 mm across and 4.9 mm along (the sill rebuilt to ±5 mm: the top's first ray
  starts on the face's plane), the body turned to -1.1075 degrees as the game's, the stance at the start 0.578390
  against 0.578389, the approach 0.35869 s against 0.36195. The second mount stands where the game's floor is
  2.7 cm lower, which the replay's one floor cannot hold: the stance 0.578 against 0.623, the approach 0.420
  against 0.409.
- **The bipod** (`06-bipod`, the M60E6, folding backward): the gesture `33` shown on the press's frame (2.502 s)
  and `34` at 12.005 s, the toggle's operation back to idling on the game's frames (3.068 and 12.680 s), the
  hands' animator's states gap 0 on every frame, both shots fired on their frames, no sound in either (the
  M60E6's hands have no bipod clips). **Open**: through the gesture the weapon's place follows the game's
  within 2 cm (1 mm at most frames) but its turn is up to 7.9 degrees apart (the game's about 0.2), the body's
  carry of the weapon in a gesture (its weight 0.9 in ours where the game's curve is 1), not the bipod's; and
  lying down came 3 frames late (1.425 against 1.400 s) on this run, before any of the bipod.
- **Everything else** on the five runs on open ground: the movement state, the stance, the lean and the body's
  `Tilt` gap 0 on every frame but the M60E6's lying down above.

**Recorder 0.10.0's `courses2\4-mount`** (the M700, by hand, two forward mounts held), the rules set on what it
recorded (`bin/mount`; `local\mount_m700_d_s132.txt`): **the data to the float** on both mounts (the target
within 0.1 mm; the look's target yaw; the yaw limits 153.485 to 191.437 and 146.761 to 184.713, so our pivot,
from the left hand's marker, is the game's; the pitch and stance limits; the approaches 0.45058 and 0.50715 s;
the stance at the start); **the approach** on all 54 and 60 frames (the share, and the look by the lerp of a
lerp, gap 0.0000); **held**, the stance by the pitch on 955 and 1,455 frames within 0.00001 and the lean by the
yaw within 0.0001; **the exits** 0.2000 s with the share falling as the rule's; **the sounds**: the kit's medium
rattle on the mount's frame, at the exit's start and 0.2 s later, and the weapon's mount sound at +0.4584 and
+0.5083 s, our timer's to the frame.

**Captures** (`local\shots\`): standing at the range's window (a 0.9 m sill) the search refuses, as the game's
rule does with the body's root 1.41 m up (`mount_window_before_s132.png`); crouched it mounts, the character
stepped up to the sill and the weapon low over it (`mount_window_s132.png`), and aimed the eye over the rail with
the handguard just over the sill (`mount_window_aim_s132.png`). From outside not taken: the range has nothing at
mount height with room for the watching camera.

**The bipod's `_procedural` nodes**. **Measured holstered**: in `courses9\fp-mounted-akm` and
`courses9\outside` (recorder 0.15.0, 8.7 and 10.0 s) the M700's Atlas hangs on the back, and each of its six is
written once, at first sight, with the prefab's values (`axis_left_procedural` at 0.0241, -0.0004, -0.0076,
`leg_left_procedural` at 0.0042, -0.0909, 0, `axis_horizontal_procedural` at z 0.0207), and never again (the skeleton
writes a node only when it moves); in `courses8\m4a1-left-shoulder-canted` the holstered M60E6's two keep one
value. **In hand, by eye only**: `m700-bipod`'s pictures 13, 25 and 27 (the prone mount's entry, turned to +15 and
to -14.8 on it) show the Atlas's legs keeping one relation to the stock through the pan, no axis panning or canting
against the weapon; ours (`local\shots\m700_prone_s165.png`) has the leg where picture 13 has it on the screen, by
eye. No recording has a bipod in the hands' rig (`m700-bipod` is recorder 0.14.0, before the rig was written).
**Tested**: the seven bipods' clips bind none of them (`install::no_bipod_clip_binds_a_procedural_node`, trp-data);
on all 180 frames of the Atlas going out and folding on the M700 (the build of `m700-bipod`) each of its six stands
at its rest within 1e-6, while the left axis turns 90 degrees and the left leg slides 5.95 cm
(`mounting::the_bipod_s_procedural_nodes_stay_at_rest`); the RPD's hands' animator alone puts its bipod's four where
the rest puts them under the slot, within 1 µm and 0.0002 degrees, its root turned 90 degrees from the rest
(`mounting::the_rpd_s_hands_hold_its_bipod_at_rest`).

**Read from code only, not recorded**: a standing bipod mount; the prone one's body (`Prone_Bipods`) and the
tactical blend; every exit but the key's, the walk's lock, the walk keys and a stance key (the lean, lying down,
the jump, blind fire, a reload, the looks, the fold, the inventory, the backpack); the recoil on a side mount; the
breath and sway on a mount; the arms' drain; the skill's ergonomics gain above level 0; the dynamic lower limit;
the turn's collision check; the canted sight's swap; the bipod's refusals; the standing search against the
world it meets (the colliders' record of `courses8` could rebuild the corner for it); third person.

## Not built, or not read

- (The tactical reload's stiffening, which gave the SV-98 a different chambering animation on a mount: built 0.163.1 and compared with `local\traces\m700-bipod`'s three shots, `firing.md`,
  "The reload with the sights up". The SV-98's controller has no mounted state of its own: the difference was
  the weapon held toward its rest while the bolt is worked, where ours had swung with the clip.)
- (The bipod's `_procedural` nodes: read 0.224.1, moved by nothing, "The bipod". The bipod's legs and sounds: built
  0.144.0 and 0.145.0. The battle screen's mount icon: built 0.126.0.)
- **The RPD's bipod out, in first person**: its hands' clips hold the bipod's inner joints folded on every frame
  while its own animator unfolds them; ours lets the bipod's win. Which of two animators writing one node
  Unity lets win was not read, and no recording has the RPD's bipod out.
- A door or a moving platform under the point; a stationary weapon (the left stance turned off at the mount: built
  0.127.0, `left-shoulder.md`);
  the left hand's item; a bot; the `MountingPacket` and others' view of it.
