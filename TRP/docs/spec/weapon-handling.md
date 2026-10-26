# Weapon handling

How the weapon and the camera move in first person beyond what the animation clips do: sway, lag, bob, aiming. Written from a reading of the game (0.16.9.5, SPT 4.1.6 assembly). Class and field names are the game's. Items marked **unread** have not been studied yet.

## Where it lives

- `ProceduralWeaponAnimation` and `PlayerSpring` are components on the `Base HumanRibcage` bone of the player prefab (`assets/content/commonprefabs/playersuperior.bundle`). Their serialized values are the tuning data; they are read from the bundle.
- The ribcage is turned to the look direction, so it is also the `TrackingTransform` the motion effector watches.
- The weapon prefab supplies the bones acted on: `Weapon_root` (moved by clips), its child `Weapon_root_anim` (moved by this system; the game calls it `WeaponRootAnim`), then `weapon`. `Camera_animated` is the clip-driven camera bone (`CameraAnimatedFP`).
- When a weapon is taken in hand (`InitWeaponData`) its item template in `items.json` overwrites fields on `PlayerSpring` and on the animation: `RotationCenter`, `RecoilCenter` (the game's `RecoilPivot`) and `RotationCenterNoStock`; the recoil values (see Recoil); `CameraSnap`; the breathing values. The aim planes come from the `WeaponPrefab` script. The same call sets the hands position spring's `Damping` to `Aiming.RecoilHandDamping` (0.45), replacing the prefab's 0.5.

Axes: everything below acts in the space of `Weapon_root_anim`'s parent, the weapon's bone space, where x is right, -y is along the barrel and z is up. A rotation about y is therefore a roll about the barrel and a rotation about z is a yaw.

## Per frame

`Player.VisualPass` calls `ProcessEffectors(time, fixedFrames, motion, velocity)`. `motion` is the movement code's input velocity before collision (`InputMotion`), `velocity` the resulting velocity.

**The time it is given.** `fixedFrames` is the number of physics steps since the last rendered frame. When that is above zero, `time` is those steps' total (a sixtieth of a second each); when it is zero, `time` is the rendered frame's own length as the player's update was given it (`Player._armsTime`, the arms' update time: the player's stopwatch read in whole milliseconds, the recordings' `clock_dt`, as the movement's own; not the engine's `Time.deltaTime`). Until 0.110.0 the simulation took the engine's there; measured since (`courses4`'s M4A1, a canted sight's turn: the game's share on such frames is the stopwatch's 9 or 8 ms, not 8.33), and the regression's camera and eye came closer in 92 and 89 scenarios. The effectors and springs run once per physics step and not at all on a frame without one. **The aim reaches the weapon's motion on its own frame**: the firearm's controller sets `ProceduralWeaponAnimation.IsAiming` as it changes its aim, in the frame's update, so the late update of a frame with no physics step already goes by it (until 0.110.0 the simulation told it with the next physics step's input, and on such a frame the camera's ease and a canted sight's turn began a frame late: `courses4`'s `03-fire-aimed`, 4.2 degrees on its first aimed frame). Everything after them (applying to the weapon, the camera's approach to its target, the camera following recoil) runs every frame with `time` as its step. So at 60 frames a second those see real time, and above that they see more: at 120 frames a second, 1/60 on one frame and 1/120 on the next, one and a half times the clock. The aiming camera is that much quicker at high frame rates. A trace recorded at 120 frames a second shows exactly this alternation, and the simulation reproduces it.

**What does and does not follow the frame rate, and what a host does about it.** The springs do not: 60 steps a second at any rate, so recoil's climb and return are the same. The game does not draw between their steps either: in a 120 frames a second trace the hands' springs and the weapon bone change on every other frame only. The camera's code does follow it, as above: bringing the sights up, the camera following the muzzle, and the field of view's easing (a step per frame) all run faster and unevenly above 60 frames a second. The weapon's logic runs once a frame, so a shot or a reload's stage lands on a frame boundary. A host therefore does not call any of this with the frames it draws. It runs the whole frame loop on a clock of its own (`trp_sim::Pacer`: 120 frames a second, the rate of the recordings, with a physics step in every other frame), which gives the game's results at that rate, and draws by blending between the last two frames that had a physics step. The blend is smooth at any rate of drawing and one physics step behind. It is the one thing here that the game does not do.

1. For each physics step that elapsed since the last frame (the effectors and springs run at the engine's fixed timestep, not the frame rate):
   1. Motion reaction: `FixedTracking`, then `Process`. (`FixedTracking` smooths the movement terms with the rendered frame's time, not the step's, so above 60 frames a second they respond more slowly.)
   2. Force reaction: see "Force reaction" below.
   3. Breathing. **Unread.**
   4. Walking, if enabled.
   5. Hand shake. **Unread.**
   6. Turn-away from obstacles. Read, **not built**: see "Turn-away" below.
   7. Step the springs: hands position, hands rotation, sway, recoil, camera rotation, camera position.
2. Apply to the weapon: zero adjustments, aim weight, pose blends, position, rotation, tactical reload, obstacle avoidance.
3. Apply to the camera: target position, then `LerpCamera`.

Each stage is gated by a bit of `Mask`. The prefab's value is 125.

## Springs

`Spring` (hands position, hands rotation, camera rotation, camera position). Three components. State: `Current`, `Velocity`. Per fixed step, with no time step in the formula:

```
if forces were added since the last step:
    Velocity += (clamp(Force, ±AccelerationMax per component) + ForceLimitless) * InputIntensity
    clear the forces
Velocity -= (Current - Zero) * ReturnSpeed
Velocity *= Damping
Current  += Velocity
```

The value read is `Zero + Current`. Since `Current` itself is drawn to `Zero`, an offset held in `Zero` counts twice once the spring has settled: a full lean puts 5 in `Zero` and the weapon and the camera end up rolled 10 degrees. A recorded trace confirms both: `Current` settles at 5 and the camera's roll at 10.

| Spring | ReturnSpeed | Damping | AccelerationMax |
|---|---|---|---|
| HandsPosition | 0.1 | 0.5 | 10 |
| HandsRotation | 0.066 | 0.66 | 10 |
| CameraRotation | 0.05 | 0.5 | 100 |
| CameraPosition | 0.25 | 0.66 | 10 |

`InputIntensity` of both hands springs is set to `IntensityByAiming²` when aiming or stance changes. `IntensityByAiming` is 1 from the hip and `Aiming.AimProceduralIntensity` (0.75) while aiming. The same change sets the recoil rotation's `Intensity` to `IntensityByAiming` and rescales the sway factors.

`BetterSpring` (sway). A damped harmonic oscillator integrated exactly over the step, with position and velocity as state. The prefab says `angularFrequency` 7 and `dampingRatio` 0.8, but the game replaces both from the settings object whenever the character's condition is refreshed: `lerp(SWAY_FREQ_NORMAL_DAMAGED)` and `lerp(SWAY_DAMPING_NORMAL_DAMAGED)` over arm damage plus overweight. For a sound, unburdened character that is 8 and 0.8, and with 8 the simulation's sway in a recorded turn is within 0.13 degrees of the game's (with 7 it was 30% too large). The underdamped case:

```
ωζ = ω * ζ,  α = ω * sqrt(1 - ζ²),  x = pos - equilibrium
e = exp(-ωζ * dt),  c = cos(α dt),  s = sin(α dt),  b = (vel + ωζ x) / α
pos = equilibrium + e * (x c + b s)
vel = -e * ((x ωζ - b α) c + (x α + b ωζ) s)
```

Effectors add to its velocity directly.

## Motion reaction (`MotionEffector`)

Tracks how the view turns and how the player moves, and feeds both into the springs.

**Tracking, per fixed step:**

- Movement: `motion` with its y clamped to `±|velocity.y|`, converted into the tracking transform's axes, is the target of `PositionVelocity = SmoothDamp(…, VelSmooth)`. `PositionAcceleration = SmoothDamp(…, PositionVelocity - previous PositionVelocity, AccSmooth)`. Both smoothing times are 0.05 s.
- Turning: the "gyro" is how far the tracking transform's forward moved since the last step, in its own axes: x is the yaw change, positive to the right; y is minus the pitch change, positive looking up. Divided by the step it is degrees per second. Then, in this order:
  - `RotationVelocity` is clamped to `±RotationInputClamp` (300) per component;
  - `rotAccSum = lerp(rotAccSum, RotationVelocity - previous RotationVelocity, 1/3)`;
  - `rotVelSum = lerp(rotVelSum, gyro rate, 0.2)`;
  - `RotationVelocity = SmoothDamp(RotationVelocity, rotVelSum, smoothTime 0)`, `RotationAcceleration = SmoothDamp(RotationAcceleration, rotAccSum, smoothTime 0)`. A smoothing time of 0 is treated as 0.0001 s, so these follow almost immediately.

**The tracking transform is a frame behind the view.** It is the root of the weapon's rig (`HandsContainer.TrackingTransform`, set to the weapon hierarchy's own object when a weapon is taken). The game turns that object to the view once a frame, in `Player.MouseLook`, and `Player.VisualPass` calls `ProcessEffectors`, which works the frame's physics steps, *before* it calls `MouseLook`. So a physics step worked in a frame sees the rig as the frame before left it: the gyro, and the axes the movement is put into, go by the view of the previous frame. At 120 frames a second and 60 steps that is half a step's turn that reaches the sway a step later than the view shows it. Read from the code and recorded (eleventh recording, 58: flicks at 420 degrees a second, where the sway swings 7 degrees): with the view taken a frame late the simulation's sway is the game's to five decimal places on every frame of the flick (-0.01179, -0.08218, -0.23664 degrees on the first three steps); taken from the frame itself it led by a frame and was up to 0.83 degrees out, 0.07 to 0.28 in the slower turns of the older recordings, which had passed as agreement. Built: `Hands::turn_rig`, which a host calls once a frame after `advance`.

**Processing.** Each parameter set is a list of (source, target, component, intensity). Sources: 0 mouse x velocity, 1 mouse x acceleration, 2 mouse y velocity, 3 mouse y acceleration, 4 to 6 movement velocity x, y, z, 7 to 9 movement acceleration x, y, z. Targets: 0 camera rotation, 1 hands position, 2 hands rotation.

- Movement entries add `source * intensity * dt * Intensity` as a clamped force to the target spring. `Intensity` is 0.45 (times 0.15 on a bipod).

  | Source | Target | Intensity |
  |---|---|---|
  | move x velocity | hands rotation y | -5 |
  | move x velocity | hands rotation z | -7 |
  | move z velocity | hands rotation x | 2 |
  | move x velocity | camera rotation z | -1 |
  | move y acceleration | hands rotation x | 133 |
  | move x velocity | camera rotation z | 2 |

- Mouse entries all go to the sway spring's velocity, whatever target they name: `Magnitude(source) * SwayFactors[component] * intensity * dt`. `Magnitude` is a curve that is roughly `0.12 * x` below 25 °/s and steepens to ±100 at ±240 °/s.

  | Source | Sway component | Intensity |
  |---|---|---|
  | mouse x velocity | z | -1 |
  | mouse y velocity | x | 1 |
  | mouse x acceleration | z | -22 |
  | mouse y acceleration | x | 22 |
  | mouse x velocity | y | 7 |

**Sway factors.** `ergonomicWeight = weaponWeight * (1 - sqrt(totalErgonomics) / 25)`. `swayStrength = SWAY_STRENGTH_PER_KG(ergonomicWeight * (1 + Overweight))` and `displacementStr = DISPLACEMENT_STRENGTH_PER_KG(same)`, both curves from the settings object. `SwayFactors = (s, s or 0.3 s when aiming, s) * IntensityByAiming`.

**The skills' share** (since 0.103.0; `skills.md`): the weapon class's ergonomics buff (0.002 a level) on the ergonomics all of this goes by (`TotalErgonomics`: its own times one and the buff), Aim Drills' draw speed (0.01 a level) on the aiming speed, and the class's recoil suppression (0.003 a level) off the recoil's forces (`max(0, parts × (1 − x))`, before the aiming settings' bonus is added); each taken as the weapon is drawn, 0 untrained. (The recorded character's 0.87 s to aim against an untrained 1.16 s is its Aim Drills: a level of about 33 then, 40 in the profile now; not compared.)

**When the weapon's weight is taken.** The ergonomic weight, and with it the two strengths above, the aiming speed and the aim sway's strength, are worked out in `UpdateWeaponVariables`, which runs when the weapon is drawn, when it is modified (`WeaponModified`, which the end of a reload also calls) and when an arm's condition changes. Nothing runs it when a round is fired. So a weapon that has fired 24 rounds since its last reload still sways and comes up to aim as the full one did: the weapon's weight has dropped 0.29 kg and the motion has not heard of it. Recorded (eleventh recording, 22, which follows the firing scenarios and precedes the reload: the header's weapon weighs 3.567 kg and its ergonomic weight is still 2.880, which is 3.855 kg's): with the sway's strength taken from the weapon's present weight the simulation's sway was 4.3% weak on every frame of that scenario; taken from the header's ergonomic weight it is the game's. The app loads a full magazine at every reload and never lightens the weapon between, so it has this right as it stands; the replay takes the recorder's ergonomic weight.

## Force reaction (`ForceEffector`, the prefab's `ForceReact`; `GrenadeShake_CO`, `GrenadeShake`, `Process`; read and built 2026-10-18)

What a blast does to the view (grenade spec, "The blast": the shake's strength, by the distance). The prefab's figures: `Intensity` 1, noise reaching 2.5 about x and y and 5 about z (`_eulersNoiseXYMaxValue`, `_eulersNoiseZMaxValue`), and three values, the force's x, y and z each into the same component of target 1 at 1. **Target 1 is the camera's position spring**: the effector's `Initialize` is handed the camera's rotation, the camera's position and the hands' rotation, in that order, for its targets camera rotation, hands position and hands rotation.

- **The shake** (`GrenadeShake_CO(strength)`, a coroutine on the character's own weapon animation): a time left of three quarters of the strength, a power of 1.2 times it, a wait of 0.1 s. While time is left: a kick, then the wait (in seconds of game time), the time left less the wait, the power three quarters of itself, the wait a tenth longer. The first kick comes at once, as the blast is worked out.
- **A kick** (`GrenadeShake(direction, power)`), the direction a random one on the unit sphere and the power the shake's plus a draw from 0 to 0.15: the force gains the direction times `Intensity × power × 0.0005` held within 0.0003 and 0.0006, six more steps are given to it (`_framesLeft` 6), and the camera's rotation spring an acceleration of a draw within the noise's reach on each axis (−2.5 to 2.5 about x and y, −5 to 5 about z) times the power (held by the spring's `AccelerationMax` as its other pushes are).
- **Each physics step** (after the motion reaction), while steps are left (seven after a kick: from 6 down to 0): one fewer left, the sum of the force turned toward the force by a sixth (`Vector3.Slerp`: by the angle and the length; where either is nought, a straight blend, `PROVISIONAL`: the engine's own code is not read), the force made that sum, each value's component of it times its intensity pushed into its spring without the spring's limit (`AddAccelerationLimitless`), and the force put back to nought. So a kick's force is eased in and out over the steps after it.

- **A hit's push** (`AddForce(strength, hands, camera)`, from `EffectsController.OnPlayerDamaged`; read and built 2026-10-18, 0.61.0, `Hands::add_force`): health spec, "A hit and health's other screen effects". A turn drawn within the noise's reach, made of a length drawn between `1 − _deltaRandom` (the prefab's 0.3) and 1, times `WiggleMagnitude` (the globals' `AimPunchMagnitude`, 5.76) and the strength held to 1, added as an acceleration within the springs' limit to the camera's rotation spring by the camera's share and the hands' rotation spring by the hands'; nothing of the force's eased steps.

Not read: `HardShake` (a mine's).

## Walking (`WalkEffector`)

Enabled while the movement state is the running state; turned off, with its clocks reset, otherwise.

- A preset is a list of entries: a curve over one step cycle (0 to 1), a time shift, and a list of (target, component, intensity). The prefab has presets for normal, lame, sprint and duck, and a second set used in proportion to overweight. Only normal and lame are ever selected in this build, by leg injuries.
- Per fixed step, each entry advances its clock by `dt * StepFrequency`, evaluates its curve, multiplies by `Intensity`, and if the result is not zero adds `value * entryIntensity` as a clamped force to each of its targets. The clock starts at the entry's time shift. The curves repeat.
- `Intensity = lerp(min, max, speed)` with `speed` the animator's speed parameter and the range (0.5, 1.4), or (0.5, 0.7) for the duck preset.
- `StepFrequency` starts at 0.7 and is retuned at every footstep: `0.5 / clamp(timeSinceLastStep, 0.7 - speed / 2, 1.2)` while walking, `0.5 / clamp(t, 0.3, 0.8)` while sprinting, ignored if the step took 1.2 s or more. Footsteps come from the body animation, where a curve changes sign at each foot plant, at most one per 0.2 s.
- The camera rotation entries are scaled by the head-bobbing setting.

## Applying to the weapon

1. **Zero adjustments.** `HandsRotation.Zero.y = SmoothedTilt * PossibleTilt` (leaning rolls the weapon). `HandsPosition.Zero.y = 0.05` for a folded compact weapon, else 0. Blind fire adds its own offsets. `PossibleTilt` is how much of a lean reaches the weapon and camera, 0 to 1. A blender (`TiltBlender`, 5 per second) moves to 1 when the movement state becomes idle and to 0 otherwise. While it is below 1, `PossibleTilt = max(blender, tilt(speed, stance))`, where `tilt` is the height, clamped to 0..1, of the plane through (speed 0.3, stance 1) -> 1, (1, 0.1) -> 1 and (1, 1) -> 0: `(1.6 - 0.9 speed - 0.7 stance) / 0.63`, with the smoothed speed and smoothed stance. This is only worked out while a lean is asked for; between leans the last figure stands (the recorded game shows it at 1 throughout a walk with no lean). So the lean reaches the weapon fully when standing still, slow or crouched, and not at all walking upright at full speed.
2. **Position.** `Weapon_root_anim.localPosition = HandsPosition value + recoil position terms`.
3. **Rotation.** `r = HandsRotation value + sway + displacementStr * (0.25 when aiming, else 1) * (sway.x, 0, sway.z)`. The bone is rotated about `RotationCenter` (a point in the bone's own axes; the mounting centres when mounted, the no-stock centre for a folded compact weapon): first about y by `r.y`, then about x and z together by `(r.x, 0, r.z)`, with the bone's position swung about the same point. Recoil rotation is then applied about `RecoilPivot` (see Recoil), and aiming alignment about the muzzle ("The sights" below; read and built on 2026-10-12).
4. Between the rest values and the position (steps 1 and 2) the first-person poses of body states are added to the rests ("First-person poses of body states" below).
5. Tactical reload stiffening (read: `firing.md`, "Aiming, and what takes it away"; not built) and obstacle avoidance (read under "Turn-away" below; not built).

## Camera

The camera is not a child of `Camera_animated`. It has its own transform beside the weapon root.

- **Position target.** Not aiming: `CameraOffset + FOV offset + turn-away shift`; `CameraOffset` is (0.04, 0.04, 0.05). Aiming: the current sight's aim bone, expressed in the space of `Weapon_root`'s parent, with its forward component replaced: `forward = (origin of Weapon_root_anim + barrel direction * depth * s).z + (1 - s) * 0.075`, where `depth` is the nearer of the weapon prefab's `DefaultAimPlane` and `FarPlane` (0.08 and 0.35 for the AKM), and `s` is the field-of-view scale below. An optic uses the far plane and its own distance instead (read and built 2026-10-23, 0.107.0: `optics.md`, "Aiming through it"; the far plane is also `depth`'s bound over iron sights, less the weapon bone's own place along y: the kept recordings replay to the same figures with it). With iron sights there is also a shift onto the line through the two sights and a matching small turn of the weapon ("The sights" below; built on 2026-10-12: about a millimetre on the AKM, over a centimetre on the M3 and the Five-seveN). A recorded trace puts the aimed eye at (0.1328, -0.0064, 0.0679); the simulation gives (0.1326, -0.0038, 0.0680). The target is taken each frame right after the procedural motion has placed `Weapon_root_anim` and before the body takes the weapon ("Carried by the body" below), so the camera never sees where the body carries it: getting up from lying with the sights up, the body's clip swings the weapon 25 cm and the recorded camera stays on its aiming place to the millimetre (sixth recording, scenario 35).
- **In automatic fire** (read and built 2026-10-24, 0.116.2; `ProceduralWeaponAnimation`, `CheckAutoFire`: the firearm controller's `autoFireOn`, on from the start of a run of automatic fire to its end): the target is not the aim bone with its forward part replaced but the aim bone slid along the sight's own line by the forward distance that replacement would make: `target = bone + normalize(line) * (forward - bone.z)`, where `forward` is the aim plane's depth as above (with the field of view's compensation, the turn-away and the iron sights' shift along the view) and `line` the aim bone's own -z (`-forward`), or its -y (`-up`) for a bone named exactly `aim_camera` (the weapon's own), turned into the frame of `Weapon_root`'s parent; a line of no length leaves the bone where it is (the engine's `normalized`). The iron sights' shift up is added after. So as the recoil turns the weapon the eye stays on the sight's line instead of on the view's: on the M60E6, whose eye sits 7 cm behind the Specter's aim bone, a 4.5 degree climb would otherwise put the eye 5.5 mm above the scope's axis. On every stock weapon read the line runs along the barrel (`tests/sim/install.rs`, `a_sight_s_line_runs_along_the_barrel`). Compared with course 6's M60E6 held fire, the game's recoil put on the replay: the camera within 0.7 mm on every frame (4.7 before), the eye's height against the scope 0.22 mm rms (1.75 before).
- **Moving there.** x and y: `lerp(current, target, dt * rate)` with `rate = aimingSpeed * CameraSmoothBlender * overweightAimingMultiplier` when aiming and `CameraSmoothOut` (6) otherwise. z: `lerp(current, target, CameraSmoothTime * dt * k)` with `CameraSmoothTime` 8 and `k = 1 + HandsPosition.Current.y * 100 + turnAway.y * 10` when aiming, `CameraSmoothOut` otherwise. Then the camera position spring's current value is added.
- **Rotation.** `Lerp(Euler(-180, 0, 0), CameraAnimatedFP.localRotation * CameraAnimatedTP.localRotation, headBobbing * (1 - tacticalReload)) * Euler(CameraRotation value + head rotation) * Euler(180, 0, 0)`. `CameraAnimatedFP` is the weapon's `Camera_animated`; `CameraAnimatedTP` is `Camera_animated_3rd` on the player prefab. Each rests at a quarter turn about x (-90), so their product rests at the half turn the blend starts from, and the final half turn undoes it: at rest the camera is not turned at all. `headBobbing` is the player's setting, 0.2 to 1, default 0.2; the decompiled property is misnamed `FarAimPlane`. The clip-driven bone therefore contributes its rotation only, weighted by a setting, and never its position. Then the camera follows the weapon's recoil and the camera's own recoil is added (see Recoil).
- **Field of view.** The camera's vertical field of view is the player's setting (50 to 75 degrees, default 50; the decompiled property is misnamed `HeadBobbing`). Aiming over iron sights or a collimator sets the target to the setting minus 15, through an optic to 35. A change of target starts an easing that runs once per rendered frame: `fov = lerp(target, fov, n)` and then `n -= frameTime / T`, starting from `n = 1` until `n` reaches 0. `T` is 1 for aiming and leaving aim and 2 when a weapon is first taken in hand. Because the step is per frame, the curve depends on the frame rate.
- **Sprinting does not change the field of view.** The class has a `Sprint` property whose setter would take 10 degrees off (easing over 15, and 6 on the way back), but nothing in the game ever assigns it, and a recorded sprint holds the field of view at the setting from start to finish. An earlier version of this document and of the simulation had the drop; it is gone.
- **Weapon scale at wide fields of view.** `s = lerp(1, 0.65, inverseLerp(50, 75, setting))`. Just before each frame is drawn the game sets the local scale of the ribcage bone and of the weapon's root to (1, 1, s), and puts them back to 1 after drawing, so nothing but the picture sees it. The weapon and arms are squeezed along the view and fill about the same part of the screen at any setting. At the default of 50 the scale is 1. `s` also enters the aiming eye position above and the recoil rotation (see Recoil). The squeeze is applied once: the recorder logs the weapon bone's drawn scale as (1, 0.664, 1) at a setting of 74, where `s` is 0.664 (the middle figure is the bone's axis along the barrel), and the bone's place as the camera sees it agrees with the simulation to 4 to 8 mm in every scenario. The app scales the weapon's root by `s`, in first person only: the squeeze is the first-person strategy's, and a character seen from outside has none (`ThirdPersonStrategy.ApplyFovAdjustments` sets the scale to 1). The app's free camera squeezed the rifle until 2026-10-09, when the user saw it "shrunk" there; it showed once the app started at this install's field of view of 74.
- `CameraRotation.Zero.z = SmoothedTilt * PossibleTilt`: leaning rolls the camera too.

## Carried by the body

The weapon's own animator has no sprint state. What lowers the weapon in a sprint is the body.

- The player's skeleton has a bone for the weapon as an onlooker sees it carried: `Weapon_root_3rd_anim`, a child of the last spine bone (`Base HumanSpine3`) and so a sibling of the ribcage. The body's clips move it.
- The body's clips also carry curves that drive animator parameters of the same name. Two of them hand the first-person weapon over: `authority = Weapon_3rd * First_Person_Curve_Weight`. In the sprint clips both are 1; in the ordinary standing and walking states the weapon stays where the procedural motion put it. Others free each hand from the weapon for the body's own arm pose (`Hand_Left`, `Hand_Right`; both 0 for a rifle in the sprint) and blend the shoulders (`Shoulder_Left`, `Shoulder_Right`).
- Each frame, after the procedural motion has placed `Weapon_root_anim`, the game remembers that bone's local position and rotation as an offset, and then moves the bone, in world space, toward the body's weapon bone with the offset applied to it: position `lerp(now, body bone position + body bone rotation * offset position, authority)`, rotation `Lerp(now, body bone rotation * offset rotation, authority)`. So in a sprint the weapon is where the body carries it, with sway, bob and recoil still riding on top.
- The frame the weapon's rig hangs in is set every frame to the ribcage's position and the view's rotation. The body's weapon bone seen from there is therefore `inverse(view rotation) * body rotation * spine rotation * (weapon bone - ribcage)`, with the spine rotation the product of the animated local rotations from `Root_Joint` down to `Base HumanSpine3`, and both bones taken in that last bone's axes. Only the spine's rotation matters, not its position.
- This happens before the squeeze for a wide field of view is applied (the squeeze is on only while drawing), so the carried weapon is squeezed like everything else.

**In the body's hand** (`PlayerBones.Kinematics`). The hand-over above puts the weapon where the body's weapon bone is. A second step, after it, puts the weapon in the body's right hand:

- Two more curves of the body's clips say whose each hand is: `firstPersonRightHand = 1 - Hand_Right * First_Person_Curve_Weight`, and the same for the left. At 1 the arm is solved onto the weapon's marker as usual (`hands-on-weapon.md`); the figure is the weight of that arm's solver, so at 0 the arm is left as the body's clip posed it.
- While the right hand's figure is below 1, the weapon is moved so that its right-hand marker (`weapon_R_IK_marker`, under the `weapon` bone) lands on a blend of where it was and where the body's right palm is (`Base HumanRPalm`, the end of the right arm's chain below the ribcage): position `lerp(palm, marker, figure)`, rotation `Lerp(palm, marker, figure)`, both in world space. The moved bone (`Weapon_root_anim`) gets the rotation that turns the marker so, and the position that puts the marker there. At 0 the marker is exactly on the palm: the hand no longer goes to the weapon, the weapon goes to the hand.
- The marker is animated by the weapon's own clips, so what the weapon's animator plays still shows, but the procedural motion (sway, bob, recoil) is overridden by as much as the figure is below 1.
- The crawl's clips have `Hand_Right`, `Hand_Left`, both shoulders and both elbows at 1 with `Weapon_3rd` at 1: both arms are the body's and the weapon rides in the right hand. The sprint's clips have the hand curves at 0 for a rifle, so there the weapon stays at the body's weapon bone. Lying still has them all at 0.
- For the stock AKM the hand's hold differs from the body's weapon bone by a constant 22.8 degrees in the weapon bone's own axes and about 9 cm: the palm and the weapon bone move as one in the crawl's clips, and the difference is where this weapon's marker sits against where the clips' own weapon would be held.

Which clip the body plays:

- The body animator is `Player_anim_controller` in `assets/content/characters/controllers/player_anim_controller.bundle` (the bundle also holds a reworked copy and a backup; the game asks for this name). Its clips are generic transform clips like the weapon's, about 175 bone tracks each, in `character_animations.bundle`.
- The sprint state (`Base Layer.StateMachine_Sprint.Sprint`) blends `sprint_slow_340`, `sprint_slow_0` and `sprint_slow_20` over (`Direct_X`, `SprintSpeed`), played at 0.8, 1 and 1.1 times their speed for the three sprint speeds. One cycle is 1.5 s at the slowest and 1.09 s at the fastest and holds two strides; the weapon's motion repeats every stride.
- A second layer, synchronised with the first (`Sync_SprintHands`), overrides the arms and the weapon bone with its own tree for the same state: by weapon type, then for a firearm by `WeapSizeModifier`, which the game sets to the weapon's width in inventory cells. Widths 1, 3 and 4 play `sprint_slow_0`; width 2 plays `sprint_slow_Hands_1st` in first person; width 6 plays `sprint_slow_weap_noMove`; widths between blend. The stock AKM is 5 cells wide, so it gets half of each of the last two. A weapon's width is its own `Width` plus the largest `ExtraSizeLeft` and the largest `ExtraSizeRight` among its parts, plus both figures of any part marked `ExtraSizeForceAdd`.
- From a walk the animator blends into the sprint over 0.201 s, starting the sprint at 0.122 of its cycle if the footstep curve is positive and at 0.338 if negative: one way in for each leading foot. Back to a walk takes 0.33 s. While blending, the curves blend too, so the weapon swings down and back up over those times.
- The additive layers that bend the body with the view (`Additive_Aiming` and others) switch themselves off in a sprint, and the first-person offsets layer is inactive, so the two layers above are the whole pose.

What the simulation does: the body's animator runs in full for the states the movement can reach (`body-animator.md`; `trp-sim/src/animator.rs`, `trp-sim/src/body.rs`), and each frame gives the spine's rotation, the weapon bone, the ribcage, the right arm down to the palm, and the curves. `Movement::carried` and `Movement::held` put the weapon bone and the palm into the rig's frame, and `Hands::weapon_transform_carried` applies the hand-over and then the hand's hold as above. The palm it is given is the body's own; the arm's solver having already drawn it toward the marker by the same figure is allowed for there. The weapon's marker comes from the host: the app reads it off the rig as the weapon's clips have it; the replay takes it from the recording where the recording has it and otherwise from the weapon's resting clip (`trp_sim::right_hand_marker`). It matters where the clip moves the marker while the body has the hand: a crawl begun again in a reload's tail, with the key still held (seventh recording, 47), had the weapon 52 mm and 21 degrees off for 0.8 s with the resting marker and has it within 12 mm and 2.1 degrees with the clip's. Which foot leads into a sprint comes out of the animator's own footstep curve, as in the game. Compared with a recorded sprint the weapon bone is within 7 mm and 0.6 degrees (root mean square over the whole scenario) and 0.8 degrees at worst; with no carry the same comparison gave 13 cm and 19 degrees. In a recorded crawl it is within 5 mm and 0.5 degrees, 3.7 degrees at worst during the tenth of a second in which the hold changes over; without the hand's hold it was 43 mm and 12.4 degrees.

## The frame the view hangs in

The weapon's rig, and with it the camera, is placed every frame at the body's ribcage bone and turned to the view (`MovementContext.DefaultRotationFunction` on `PlayerBones`). So everything the body's animation does to the ribcage reaches the eye:

- Standing, the ribcage is 1.476 m above the feet and the eye, at `CameraOffset` from it, 1.51 m.
- Crouching lowers it 0.60 m; a full lean carries it 0.29 m aside and 0.06 m down; each stride of a walk moves it between 1.41 and 1.46 m; a sprint lowers it further. These are the body's clips and its additive lean layer, not separate rules.
- The body's own camera bone (`Camera_animated_3rd`) is the second factor in the camera's rotation above.
- The body's height lags behind sudden vertical movement (`body-animator.md`), which is what the eye does in a jump.

Compared with the recording the eye is within 1 to 5 mm (root mean square) and 2 cm at worst in every scenario on level ground, and within 2.8 cm through a standing jump.

- **A jump** carries the weapon too, through the same hand-over: the jump clips swing it up 11 degrees at the push-off and again on landing. Through the flight of a recorded standing jump the simulation is within 0.6 degrees of the game on every axis. The landing's swing comes 0.03 s later in the recording, where the player came down 4.5 cm lower than the take-off; with a swing that fast the two are up to 8 degrees apart for those frames. Whether any of that lag is the game's own needs a jump recorded on level ground.
- Not covered: sprinting while turning (`Direct_X` and the `Sprint_Offset` lean), the hands leaving the weapon, and the shoulders.

## The camera's collider (`PlayerOverlapManager`, `ColliderExtrusion`; read and built on 2026-10-09, after the eighth recording showed it)

The camera the weapon motion places is kept out of the world by a box of its own.

- **The box.** The player prefab's `Cam` object is the transform the camera's easing moves ("Camera" above). It is a child of `CameraContainer`, which sits beside the ribcage under the last spine bone and is turned to the view every frame. `Cam` carries a box collider and a script that pushes that box out of things. The box is 0.13 m wide, 0.23 high and 0.23 deep, and its centre is (0, -0.0674, -0.05) from the camera in the camera's own axes (x right, y up, z ahead): it reaches 6.5 cm to each side of the eye, 4.8 cm above it and 18.2 below, 6.5 cm ahead and 16.5 behind. It turns with the camera, roll and all.
- **The push** (`ExtrudeCamera`). It is the last thing in the player's late update: after the weapon motion has placed the camera, after the arms are solved and after the body has taken the weapon. Only the local player has it, never a bot. Every collider near the box on the layers the game names for it (the low-poly world, doors, the terrain and other players; the character's own capsule is left out) is asked, through the engine, for the shortest move that takes the box clear of it. Each of those moves is lengthened by a millimetre, they are added up, and the camera is moved by the sum, in the world's axes.
- **It feeds back.** What is moved is the camera's own transform, and the next frame's easing starts from wherever that is. So a camera whose target is inside the ground does not sink and bounce: each frame it eases a little of the way down and is set back on the surface.
- **The picture's camera follows it** (`FirstPersonCameraState`). The camera that renders is a separate object. Each frame it takes the camera bone's rotation and goes from where it was toward where the bone is in steps of a centimetre, trying the same box at each; at the first step where the box is in something, it is moved clear of that and stops for the frame. A move of more than a metre, or of nothing, goes straight to the bone. Over even ground or along one flat wall the bone is already clear and so is the straight way to it, and the two are in one place. Round a corner they need not be.
- It is off for a player riding the armoured vehicle (`IgnoreCameraCollider`).
- **The other half** of the same manager is a capsule about the head (`playercollider`: radius 0.22 m, 0.7 m long, with a second shape for lying down). The movement code asks it how far a move, or a turn lying down, would take the head into something and pushes the move back by that, level only (`LimitMotionXZ`, `RotationOverlapPrediction`): built since 0.132.0 (`movement.md`, "The head's push out of walls").

**What it does on open ground.** Standing and crouched, nothing: the box's lowest point is 18 cm under an eye that is 0.9 m up or more. Lying down the eye is 0.3 m up and clear. Rolled onto a side with the sights up it is not: the eye goes to the sight of a rifle that lies canted close to the ground, 5 to 11 cm over it, and the box, tipped 10 degrees with the camera, has its lowest corner 19.3 cm below the eye. So the eye stops there and looks over the sight instead of through it.

**How high the ground is matters, and it is not where the feet are.** The character stands as much as 8 cm over the ground ("What counts as on the ground" in `movement.md`), and lying down it floats there too. The box is pushed out of the terrain itself. A host answers for its real surfaces; a recording has to say how far under the feet the ground was (below).

**Recorded** (eighth recording, 51: lying, the sights up, a roll to the left and one to the right). Before this was built the simulation's eye sank to 4.8 cm over the feet in the roll to the right and 11.0 in the roll to the left; the game's never goes under 11.5 cm in either, and holds there level to a tenth of a millimetre while the roll's clip carries it sideways. Worked back from the box, turned as the recorded camera is (its lowest corner 19.3 cm under the eye), that puts the ground 7.6 to 7.7 cm under the feet; the same recording's standing jump from the same spot lands 7.62 cm down. With the box in and the ground put there, the eye through both rolls is within 3.5 mm of the game's at worst and 1.1 mm root mean square (it was 67 mm at worst), and the height it rests at is the game's within 0.1 mm. The box is pushed on 283 of the rolls' 760 frames. In the roll without the sights (38, in three recordings) the eye stays clear of it, as before.

**Built.** `trp_data::player::CameraBoxData` reads the box. `Movement::camera_push` puts it in the world from the frame the rig hangs in and the camera's pose, asks the host how far it has to move (`World::push_out`; a host that says nothing gets the ground alone, lifted along its normal by the deepest corner), and gives the answer in the rig's frame; `Hands::push_camera` moves the camera by it, last in the frame, so that the next frame's easing starts from there. The app's test range answers for its ground and its boxes (the shortest way out of each, by the boxes' faces and edges). The replay's ground is what the character rides on; it meets the terrain a figure lower, the third number of a recording's `ground.txt`, taken from a standing jump's landing. Not built: the picture's camera stepping after the bone (it only differs round corners). The head's capsule is built since 0.132.0 (`movement.md`).

**Measured in the app** (level ground, the character standing on it): in an aimed roll the eye rests 19.2 cm over the ground on either side (in the roll to the left it was 10.3 cm before); standing and lying from the hip the box touches nothing (`cargo test -p trp-sim --test sim the_cameras_box -- --nocapture`).

## Aiming speed


From the weapon and `globals.json` `config.Aiming`:

```
t   = inverseLerp(LightWeight, HeavyWeight, weaponWeight)
max = lerp(MaxTimeLight, MaxTimeHeavy, t)
min = lerp(MinTimeLight, MinTimeHeavy, t)
e   = clamp01(totalErgonomics / 100)
k   = 0.25 + 3 e²  if e < 0.25,  else 2 e - e²
aimingSpeed = 1 / lerp(max, min, k) * (1 + skill bonus)
```

Here: light 0.6 kg, heavy 9 kg, max time 2 to 2.4 s, min time 0.35 to 0.95 s.

## Aim sway (`_aimSwayStrength`, `_aimSwayDirection`, `_aimSwayBlender`; read and built 2026-10-24, 0.113.0)

A heavy weapon swings off and back as its sights come up. **Its strength** (`UpdateWeaponVariables`) is `inverseLerp(AimSwayStartsThreshold, AimSwayMaxThreshold, weight × (1 − clamp01(totalErgonomics / 100)))`, the prefab's 3 and 7 kg: none on a rifle (the AKM's 3.87 kg at 36 ergonomics is 2.5), whole on the M60E6 (12.3 kg at 14.55: 10.5). **As the sights come up** (`IsAiming`, set to true), where the strength is above nought, the direction is drawn afresh: the strength times the stance's intensity (`config.Aiming.ProceduralIntensityByPose`, the pose's) times three of Unity's `Random.Range` (x, then y, then z) between the prefab's `AimSwayMin` (−0.33, 1, −0.33) and `AimSwayMax` (−1, 6, −2) (not the code's defaults); and the blender is set whole. **Each rendered frame** the sights are up and the blender is above nought, in the camera's frame (`LerpCamera`; mounted, `MountedWeaponCamera`), the sway spring's velocity is given the direction times the blender (`SwaySpring.ApplyVelocity`): a kick a frame, not a physics step, so a higher frame rate kicks more often. **The blender** (`Player.ValueBlender`, target 0) goes down from 1 by `SwayFalloff / aimingSpeed` a second by the game's clock (`Time.time`): the prefab's 1 over the aiming speed, so it lasts as long as the sights take to come up. The random draws come from Unity's own generator, shared with the rest of the game, so a replay takes the direction from the recording (recorder 0.12.0 records it; `replay --aim-sway x,y,z` gives one).

**Compared** (`courses4`'s M60E6, `01-aim`, recorded before the direction was): the direction that best fits the recorded weapon root's turn over the aim-in, by least squares on the replay's response to each unit direction (`local\fit_sway_s122.pl`), is (−0.70, 4.76, −2.06), inside the prefab's box at a stance intensity of 1 (z 3% past its end, within the fit); with it the turn's rms gap over the script goes from 1.88 to 0.76 degrees (its largest from 6.0 to 1.5) and the eye's place's from 3.6 mm to 0.3 mm (its largest from 11.7 to 1.2 mm). The other scripts' directions were drawn anew and are not on record.

## The sights (`FindAimTransforms`, `method_14`, `method_15`, `ApplyAimingAlignment`; read and built on 2026-10-12, after the user saw the Five-seveN's sights and the folded M3's off)

Until then the eye went to the weapon's own aim bone, which is right only for a weapon with no sight on it.

**Which bone the eye goes to.** Each sight fitted gives the bones named `mod_aim_camera` below its own bone (not those of a scope fitted on it in turn), each kept only if it is turned no more than 55 degrees from a half turn about the weapon's upright axis (`ScopeRotationWithinLimits`). If the sights give none, the first `mod_aim_camera` anywhere on the weapon that is within that limit is taken; if there is none, the weapon's own `aim_camera`. So a weapon with iron sights is aimed by its **rear sight's** bone, and a weapon with no sight at all by its own. Where the build has several (an optic and its back-up sights), a key chooses: read and built 2026-10-23 (0.107.0; `optics.md`, "The sights a weapon has"), where the whole rule is: the sight parts' own bones first, and the iron sights only where no sight part looks straight ahead.

**The line through the iron sights** (`method_15`). The rear point is the bone whose name contains `mod_align_rear` that lies deepest below the weapon's own bone, the front point the deepest `mod_align_front`. A weapon with both has a line; with either missing nothing below applies, and the eye is the aim bone's.

**Measured as the sights come up** (`method_14`; cleared as they go down, `method_12`). In the weapon's own bone, seen from the side (there the barrel runs along -y and up is z):

- the line's pitch from the weapon's own way, `atan(rise / run)` from the rear point to the front point;
- the eye's shift: the point of the line as far behind the rear point as the aim bone is behind it, less the aim bone. Its first figure is along the barrel, its second up.

The same call works out the direction a shot leaves in for the sights' range (`CalculateShotDirectionForIron`): not built, with the rest of ballistics.

**The eye.** The aimed camera's target ("Camera" above) takes the shift: the second figure onto its height, the first onto its forward figure. Sideways it stays the aim bone's. So the eye sits on the line through the two sights.

**The weapon.** Once a frame in the late update, after the recoil's turn and before the turn-away, a figure goes a share `dt * 5` of the way to the line's pitch while the sights are up and to zero while they are down (`dt` as for the camera: the physics time on a frame that had physics steps, the player's own otherwise, "Per frame"). Where it is over 0.002 degrees the bone the procedural motion moves is turned by it about its own x, through the muzzle (the fireport's place). So the barrel is tipped until the sights' line lies along the view.

**A canted sight** (`UpdateCurrentScopeRotation`, `ApplyComplexRotation`; read and built 2026-10-23, 0.110.0). Each sight listed keeps its turn from straight ahead, `180 − the y Euler angle of its bone's turn relative to the weapon bone` (a sight past 55 degrees is not listed, `ScopeRotationWithinLimits`). The sight in use gives the weapon a target turn about its root's own y (the barrel) of that many degrees, where it is at least `SCOPE_ROTATION_THRESHOLD` (1), else none; worked out as the sights are found and at each change of sight. Each frame, after the sights' line, the turn goes by `Quaternion.Lerp` (blended and renormalised) a share `CameraSmoothTime × aiming speed × dt` of the way to the target while aiming, to none while not, and is put on the root's turn on its right (`_temporaryRotation * _scopeRotation`), its place kept; the turn-away after it. The eye then goes to the sight's bone where that turn put it, so the canted sight comes upright before the eye. The PK-06 on a 45 degree offset mount (`5649a2464bdc2d91118b45a8`, its own 45 degrees whether on the top rail or the handguard's side) is turned −45. **Compared** frame for frame with `courses4`'s M4A1 (the user's build, `replay --recorded-build`): the weapon root's turn about its barrel, each trace's rotation spring's own turn about y taken off, within 0.007 degrees in `01-aim` (rms 0.002), 0.004 in `03-fire-aimed`, 0.006 in `05-walk-aimed` (with its own build: a 30-round magazine, aiming speed 2.049); before the two timing rules of "Per frame" the gaps were 0.26 and 4.2 degrees in the first two; the third's was 9.9 with the first's build (its 100-round drum: an aiming speed of 1.335, not 2.049). Not built: the left stance's turn (`LeftStanceRotation`, 190 less the angle), the zeroing's use of the turn (`GetWorldCalibrationPoint`, with step 4 of `optics.md`).

**What the install's four weapons have** (the rigs at rest; `the_eye_is_lined_up_with_the_iron_sights`):

| Weapon | Aimed by | The line's pitch | The eye, from the weapon's own aim bone |
|---|---|---|---|
| AKM | the rear sight's bone | -0.023 degrees | 1.0 mm |
| M3 Super 90 | the ghost ring's bone | 0.010 | 13.3 mm |
| FN Five-seveN | the rear sight's bone | 0.024 | 12.0 mm |
| Remington Model 700 | its own `aim_camera`: the stock preset has no sight | none | 0 (the bone lies 7.8 mm left of the bore and 5.8 cm above it) |

**Built:** `trp_sim::rig::SightNodes` (the bones found in an assembled weapon; the line and the muzzle read off the rig as its animator poses it), `Hands::note_sights` (told every frame; the line is taken as the sights come up), the shift in `Hands::camera_position` and the turn in `Hands::weapon_transform`; the replay and the app both use them. The choice between several sights and an optic's own distance are built since 0.107.0 (`trp_sim::sights`, which `SightNodes` now goes by; `optics.md`).

**Compared with the game:** see "Compared with the running game" below (the sixteen kept recordings, the M3's aimed scenarios among them). The Five-seveN and the Model 700 are on no recording: for those two the rule is the code's, read and built, and what it gives is not measured. The Model 700 aimed with no sight looks over the receiver from a little to the left, which is where the install's own bone is.

## Other values on the prefab

`CameraSmoothTime` 8, `CameraSmoothSteady` 8, `CameraSmoothRecoil` 3, `CameraSmoothOut` 6, `AimingDisplacementStr` 0.25, `WeaponFlipSpeed` 1, `TacticalReloadPosition` 0.95, `SwayFalloff` 1, `AimSwayStartsThreshold` 3, `AimSwayMaxThreshold` 7 ("Aim sway", above), `MotionReact.RotationInputClamp` 300, `Walk.Treshold` 0.05, `Walk.SideSpeedMultyplyer` 1.6, `Walk.BackSpeedMultyplyer` 2.

## Recoil

`ShotEffector` holds two recoil models; the prefab selects the newer one (`RecoilPipeline` 1, `NewRecoilShotEffect`). It has three springs, called processes: the weapon's rotation, the weapon's position and the camera's rotation. A fourth layer, the weapon's own curves, is described at the end.

### What a shot does

The firearm code calls `Shoot(strength)` once per shot with `strength = 1 + ammoRec / 100` from the cartridge (the stock 7.62x39 PS has 0, so 1). **It is called after the frame's pose** (read and built 2026-10-24, 0.116.2): the firearm controller marks the shot in its update and calls `Shoot` from its own late update, which the player runs after the procedural motion's (`Player.LateUpdate`: `ProceduralWeaponAnimation.LateTransformations`, then `HandsController.ManualLateUpdate`). So on a shot's own frame the weapon and the camera are placed as before it: the weapon's own curves (below) start again, and the camera's rate drops, from the next frame; the springs feel the force on their next physics step either way. Course 6's M60E6 under held fire shows it on every shot's frame (a roll of 1.3 degrees still drawn where a reset would have none). `Shoot`:

1. Sets the camera's aiming rate (`CameraSmoothBlender`) to `CameraSnap * Aiming.CameraSnapGlobalMult` and holds it there for 0.2 s; after that it climbs back to `CameraSmoothSteady` (8) at 5 per second. `CameraSnap` is the weapon's (AKM 3.5; the prefab's 3 is used when the weapon gives none) and the multiplier is 2, so the rate drops from 8 to 7 for the AKM.
2. Adds the recoil force, below.
3. Tells the breathing effector. **Unread.**

Adding the force:

```
shot += 1                                  (the count within one run of automatic fire)
if shot == RecoilStableIndexShot: the rotation process enters stable mode
multiplier = clamp(shot, 1, stable) / stable   if shot >= ProgressPowerShot (2), else 1

rotationPower = random(rotation range + group sums) * strength
positionPower = random(position range + group sums) * strength
angle range   = base range + group sums; in stable mode widened by ±angleAdd,
                where angleAdd grows by RecoilStableAngleIncreaseStep per shot,
                clamped to ProgressRecoilAngleOnStable
a = random(angle range)

direction = ( -sin(a) * rotationPower * Ix,
               cos(a) * rotationPower * Iy,
               positionPower * Iz ) * HandPositionRecoil.Intensity
```

The three random draws come in that order. `I` is the stance factor: `Aiming.RecoilXIntensityByPose`, `RecoilYIntensityByPose` and `min(template.RecoilPosZMult, RecoilZIntensityByPose)`, each indexed by stance: 0 prone, 1 crouched, 2 standing. "Crouched" here means the stance level is below 0.11; anything higher counts as standing. Values: x (0.6, 0.7, 1), y and z (0.7, 0.7, 1).

The ranges are set when the weapon is taken in hand:

```
parts = max(0, (1 + sum of the fitted parts' Recoil / 100) * (1 - skill terms))
rotation range = BasicRecoilRotationStrengthRange (0.9, 1.15)
                 * (RecoilForceUp * parts + Aiming.RecoilVertBonus) * 0.1399
position range = BasicRecoilPositionStrengthRange (0.65, 1.05)
                 * (RecoilForceBack * parts + Aiming.RecoilBackBonus) * 0.1399
base angle range = RecoilAngle ± RecolDispersion, in degrees
```

Both bonuses are 20. Each entry of the weapon's `ShotsGroupSettings` covers a range of shot numbers and adds to the two strength ranges and to the two ends of the angle range (in degrees, despite the field's name). The AKM has one group, for shots 1 to 200, that narrows its 74..96 degree range to 80..90. An angle of 90 is straight up.

The direction is then routed by the prefab's `ShotRecoilProcessValues`, each "this axis of the direction, times a factor, into that axis of that process":

| From | Into | Factor |
|---|---|---|
| x | weapon rotation x | 0.15 |
| y | weapon rotation z | 0.15 |
| z | weapon position y | 0.0007 |
| x | camera rotation x | the weapon's `RecoilCamera` |
| y | camera rotation y | minus `RecoilCamera` |

Two more entries on the prefab name targets the code does not handle. So a shot raises the muzzle (negative x), turns it a little sideways (z), pushes the weapon back along the barrel (+y) and kicks the camera.

### The position and camera processes

Per physics step, with no time step in the formula:

```
if a force was added: Velocity += Force * Intensity; clear it
Velocity -= (Current - Offset) * ReturnSpeed
Velocity *= Damping
Current  += Velocity
```

Weapon position: return 0.08, damping 0.5, intensity 1. Camera rotation: 0.05, 0.5, 1. `Offset` is where other effects (breathing) move the rest point; it is zero until those are in.

### The rotation process

`NewRotationRecoilProcess`. Its `ReturnSpeed` is the weapon's `RecoilReturnSpeedHandRotation * Aiming.RecoilConvergenceMult` (AKM 2.4), its `Damping` the weapon's `RecoilDampingHandRotation` (0.86), and its `Intensity` is `IntensityByAiming`. A force pushed into it is first multiplied by the weapon's `RecoilCategoryMultiplierHandRotation` (0.19) and by the shot multiplier above. In stable mode a push also zeroes the velocity first.

Per physics step:

```
if a force was added:
    Velocity += Force * Intensity; clear it
    curveTime = min(|Current|², curveTime)
    if stable:
        next = stableCurrent + Velocity * random(NextStablePointDistanceRange (0.1, 8))
        if |next - Current| > 0.95: next = Current + (next - Current) * 0.95
if not automatic fire: speed = ReturnSpeed * ReturnSpeedCurve(curveTime)
if a force was added, automatic fire, not stable: speed *= shot multiplier
curveTime += dt
Velocity *= Damping
if stable:  Current += (next - Current) * ToNextCurrentSpeed (0.2)
else:       Velocity -= Current * speed;  Current += Velocity
return path (below)
if not automatic fire and peakX != 0:
    remaining = |clamp(Current.x, peakX, -0.01) / peakX|
    k = clamp(1 - remaining, 0.01, 1) * 0.01
    Current.x and Current.z move toward the settle point by the fraction k
```

`ReturnSpeedCurve` is flat at 0.008 on the prefab, so a single shot's return rate is `2.4 * 0.008` per step for the AKM. The clamp has its bounds the wrong way round until the first shot (`peakX` starts at 0.01); the engine's clamp then returns the lower bound, and that behaviour is kept.

Stable mode starts on the weapon's `RecoilStableIndexShot`-th shot of a run (5 for the AKM) and remembers where the muzzle was. From then on the muzzle no longer climbs: each shot picks a point a random distance along the kick from the remembered point and the muzzle moves a fifth of the way there per step. Leaving automatic fire ends stable mode, resets the shot count and the angle widening.

The return path, each step:

- While the muzzle is still rising (`Current.x` below its last value) after a push: pick the settle point, x from the weapon's `PostRecoilVerticalRangeHandRotation`, sideways from `PostRecoilHorizontalRangeHandRotation` on the side the muzzle is already on; remember `peakX = Current.x`; set the sideways impulse to `|Current.x * ReturnSpeed| * RecoilReturnPathOffsetHandRotation`; and pick the side from the sign of `Current.z` (at random if it is zero).
- Otherwise, outside automatic fire: the impulse is multiplied by `RecoilReturnPathDampingHandRotation` each step and added to the sideways velocity on the chosen side until it falls below 0.001. Then the return path ends, or the whole process resets if the weapon says `ResetAfterShot`.

So the muzzle does not come back along the line it went up, and it settles near, not at, where it started.

### Onto the weapon

- Position: the position process's value is added to `Weapon_root_anim`'s local position.
- Rotation: after the sway rotation about `RotationCenter`, the weapon is turned by the rotation process's value about `RecoilPivot` (the weapon's `RecoilCenter`; AKM (0, -0.25, 0)). The game builds this turn from the three angles rotated into world axes; the simulation applies the same angles in the bone's own axes, which differs only in the second order of a few degrees.
- When the field-of-view scale is below 1, and the shot's squeeze is on (`ShotNeedsFovAdjustments`: off once a scope's optic has been turned to its zeroing point, optics spec, "Zeroing"), the x and z angles are first replaced by `atan(tan(angle) * scale)`.

### Onto the camera

Each rendered frame, after the camera's rotation is built, `AddHandRecoilRotateToCamera` blends it. `c` is the rotation process's current angles and `speed` a blend fraction kept between the weapon's `CameraToWeaponAngleSpeedRange` (AKM 0.1 to 0.2) and moved by `CameraToWeaponAngleStep * frameTime` (0.1):

- Not aiming: the camera's rotation is blended from its last blended value toward the freshly built one by the top of the range. This only smooths it.
- Aiming, not in automatic fire: target = the built rotation with its y component zeroed, times `Euler(c)`; `speed` falls; blend toward the target by `speed`. The view rises with the muzzle.
- Aiming in automatic fire before stable mode: the same with `speed` rising. If `c` is exactly zero the camera is left alone.
- Aiming in stable automatic fire: the last target is kept, with only its z component taken from the built rotation.

Then the camera process's angles are added to the camera's Euler angles. The blends are fractions per frame, so this too depends on the frame rate.

### The weapon's own curves

`WeaponRecoilSettings` in the item template, when enabled, lists curves that each drive one axis of the weapon rotation, weapon position or camera rotation. Each has a clock that a shot resets to zero and that advances by `dt * CurveTimeMultiply` per physics step; its value is `curve(clock) * CurveValueMultiply`, or `* CurveAimingValueMultiply` while aiming. The values are added to the matching process's output. The AKM has three, all the same shape (0 at 0, 2 at 0.1, -0.75 at 0.3, 0 at 1): a roll of the weapon about its barrel, a small push back, and a roll of the camera.

The game starts the clocks at zero, so the curves play once as the weapon is taken in hand. The simulation starts them finished.

### Measured (stock AKM, standing, untrained)

One shot from the hip raises the muzzle 1.6 degrees and brings the weapon back 26 mm; aiming, 1.2 degrees. Held automatic fire runs at 600 a minute; the muzzle is 1.5 degrees up when the second shot fires and about 7 to 8 degrees from the fifth on, never above 10.3.

## Breathing

`BreathEffector`, run each physics step between the motion reaction and the walk bob. Its `Intensity` is `Aiming.ProceduralIntensityByPose[stance] * IntensityByAiming`: (0.6 prone, 0.7 crouched, 1 standing) times 0.75 when aiming. With full stamina and oxygen, not holding breath, no tremor and no broken arm:

- **The breath.** One curve, repeated: up to 0.1 over the first 1.5 s, back to 0 by 3.2 s, 3.43 s in all. Each step its value times `Intensity` is pushed into the hands rotation spring's x with factor 0.33 and into the hands position spring's z with factor 0.01. At rest that dips the muzzle 0.5 degrees and lifts the weapon 10 mm with every breath; a recorded trace shows the same two figures and period. A second copy of the curve drives the camera's pitch, scaled by a factor that is 0 with full oxygen and grows as oxygen runs out; short of oxygen the breath also deepens (up to 1.5 times) and quickens (up to 2.5 times).
- **Small pushes.** Two wandering values (`SmoothRandom`) push the hands rotation spring's y and z each step. A wandering value keeps a countdown; when it runs out it draws a new countdown from its delay range and a new target within its amplitude, and turns its rate of change toward "reach the target in that time" by its hardness. The amplitude, delay and hardness come from curves over arm stamina; at full stamina 0.03, 0.25 to 0.5 s and 0.1 for y, and amplitude 0.05 for z.
- **The muzzle's wander from the hip.** Two more wandering values, amplitude 1 each, delay 0.2 to 0.7 s, give a target `(x, 0, z) * Intensity * HipPenalty` when not aiming and zero when aiming. The recoil rotation's `Offset` moves a tenth of the way to it per step, and lands on it once close. Because the recoil rotation is read as `Offset + Current`, the muzzle drifts about a degree either way when the weapon is not at the shoulder.
- **A shot from the hip** adds to a gain: `gain += max(0, perShot - 0.2 * (sqrt(gain) - 1))`. The gain holds for a delay and then falls to 0 at a rate; the weapon gives all three (`HipInnaccuracyGain` 0.16, `HipAccuracyRestorationDelay` 0.2 s, `HipAccuracyRestorationSpeed` 7 for the AKM). The wander's amplitude becomes `1 + sqrt(gain)`, at most 3, and its hardness jumps to 0.15 and eases back to 0.1. The shot also throws the offset at once to a new point drawn from the two wandering values.

**Tiredness** (read and built on 2026-10-10, when stamina existed to feed it; `stamina.md`). The effector reads two levels from the character's stamina every step:

- *How rested the hands are* (`StaminaLevel`): the arms' pool as a share of its capacity, or the lower of the arms' and the legs' where `Stamina.StaminaExhaustionCausesJiggle` is set (it is, here), never under 0.05 (`EnergyLowerLimit`). The small pushes' amplitude, delay and hardness are curves over it, and one more push comes of it: the hands rotation spring's x gets `max(0, -y's value) * (1 - level) * 2`, so tired arms let the muzzle sag in jerks. At a level of 1 that is nothing.
- *How much breath there is* (`OxygenLevel`): the oxygen pool as a share of its capacity, or the lower of that and the legs' where `StaminaExhaustionRocksCamera` is set (it is not, here). It is the input of the "short of oxygen" curve of the breath itself (above): the breath's depth is `clamp(lerp(4, 1 or 0.75 aiming, t), 1, 1.5)` times the intensity, its pace `clamp(lerp(4, 1, t), 1, 2.5)`, and its reach to the camera `lerp(2, 0, t)` times the intensity, where `t` is the curve at the level.

So a sprint, which costs oxygen as well as legs, leaves the breath a little deeper and quicker until the oxygen is back, and a spell with the sights up, which drains the arms, leaves the weapon shakier.

**Holding breath.** The key (`Breath`; Left Alt, bound as a press in this install) asks the movement state to hold the breath or let a held one go (`Player.ToggleHoldingBreath`; a binding that holds sends a second command on release, `StopHoldingBreath`). Only the standing state and the states built on it pass that on (`IdlePlayerState.EnableBreath`: standing, lying still, stepped aside, rolled over, going down and getting up; not a walk, a crawl, a sprint or a jump), and the movement context passes it on only with the sights up. `Physical.HoldBreath` then runs a drain on the oxygen: `BaseHoldBreathConsumption * HoldBreathStaminaMultiplier` taken at the legs' level, which is 4 a second on full legs and 8 on empty ones here. It does not start with the oxygen at 15 or under, and asked for on legs under 15 the call does nothing at all (so it does not start, and one already held is not let go by it). It ends when the key says so, when the sights come down (the firearm controller's aim setter), when the standing state is left with the sights up, on entering a jump, and when the oxygen runs out.

While it is held the breath's depth and the small pushes' strength are both 0.15, whatever the stance, and the breath's pace and its reach to the camera stay as the last breath drawn had them. The muzzle's wander from the hip is untouched, but the sights are up.

**Built:** `Breath::step` takes the two levels and the hold (`Wind`, through `HandsInput::wind`, from `Movement::wind`); `Stamina::hold_breath`, `holding_breath`, `breath_stamina`, `breath_oxygen`; `Input::hold_breath`, and what lets the breath go. In the app Left Alt, as bound. Measured in the simulation with the stock AKM and the sights up, over 30 s (`cargo test -p trp-sim --test sim breath -- --nocapture`): rested, a breath dips the muzzle 0.21 degrees and lifts the weapon 4.2 mm, nine breaths; held, 0.05 degrees and 0.8 mm; with a fifth of the oxygen left, 0.31 degrees and 5.7 mm, 22 breaths; with the arms spent, the muzzle sags up to 0.9 degrees. Five seconds of held breath cost 19.9 of the oxygen. **None of it is compared with the game:** no recording holds a breath, and none takes a pool low enough for the difference to show beside the breath's own random pushes (the least on record is 0.87 of the legs; what that does to the replays is under "Compared with the running game").

Not covered: the stiff arms of a stiff draw (a skill's buff: three seconds from the sights coming up in which the breath is cut to 0.3 to 1 of itself), mounting. **Tremor and a broken arm are in since 0.10.0** (`health.md`, "What health does to the character"; `Wind::tremor`, `Wind::fracture`): either replaces the muzzle's wander with three wandering values of their own (`TremorXRandom`, `TremorYRandom`, `TremorZRandom`, the player's prefab's), at the frame's pace for a tremor and half of it for a broken arm alone, aimed or not; a broken arm caps the hands' level at `EnergyFractureLimit` (0.3) and, not aimed, throws the muzzle up by the breath's roll, the more the more rested the hands. Damaged arms also soften the sway spring toward `SWAY_*_NORMAL_DAMAGED.y` (half the way for one; the carried weight's share, which the game adds, is not in), and a damaged leg without a painkiller plays the walk's lame bob (`WalkEffector.Presets`' second). From the code; compared with nothing. The hand shake effector (`HandShakeEffector`) is not tiredness: it is set up for a time after a fast weapon switch, which there is none of here.

## Turn-away (`TurnAwayEffector`; read on 2026-10-07, read again and built on 2026-10-09)

When the weapon would stick into something it is drawn back and turned aside, and past a set depth the sights come down until there is room again.

**How deep** (`FirearmController.WeaponOverlapping`, once a frame, at the end of it: after the weapon is placed, the body has taken it and the arms are solved).

- **The weapon's length** is the distance from the bone the procedural motion moves (`Weapon_root_anim`) to the muzzle (`fireport`), or to the farthest of the weapon's muzzle effects if one lies farther. Those are the objects carrying one of the game's muzzle-effect scripts, the ones deepest in the weapon's tree only (plus one named `extractor_smoke`, wherever it is). The stock AKM at rest: 0.873 m to the muzzle, 0.894 to the flash of its muzzle brake.
- **When it is measured** (`FirearmController.method_10`): when the weapon is set up, again when the weapon has appeared in the hands (the `WeapIn` mark that ends the draw, or the resting state's `IdleStart` if that comes first), and again whenever the weapon is modified. The figure in use is whichever was taken last, in the pose the weapon's bones were in at that moment. At rest the weapon's own bone sits on the moved bone; in the draw's clip (`akm_out_to_idle`) it is still ahead of it over the last tenth of a second: 11.6 mm with 0.07 s to go, 5.0 with 0.03, 2.5 with 0.01, nothing at the end, and the draw hands over to rest at 0.98 of its length, where it is 4 mm. So a weapon measured as it appears comes out some millimetres longer than at rest, by how much depending on the frame it fell on. **A reload measures it again, and not at rest** (read after the tenth recording showed the figure moving): "modified" includes the magazine changing, and the call is made as the reload's operation hands over to the idling one (`ReloadExternalMagOperation.SwitchToIdlingState`), which is at the `MagIn` mark, or at `AddAmmoInChamber` when the chamber was empty (`firing.md`, "Reloading with a magazine"). The weapon's own bone is then wherever the reload's clip has it. Recorded (tenth recording, the header of each trace, which is the figure in force as the scenario begins): 0.8973 m for the 23 scenarios before the first reload; then after each of the eleven scenarios that reload, and after no other, a new figure: 0.9035, 0.9037, 0.9043, 0.9035, 0.9014, 0.9032, 0.9034, 0.9033, 0.9034, 0.9033 after a magazine changed with a round chambered, and 0.8321 after the reload from empty, 6 cm short of the weapon. Firing, a look at the magazine and lying down do not change it. So the game's line is 3 mm longer than the weapon at rest as first drawn in that raid, 7 to 10 mm longer after an ordinary reload, and 6 cm shorter after a reload from empty until the next reload or draw. (An earlier note here said the wide field of view's squeeze changes nothing in this. It does: next paragraph.)

  **Measured squeezed** (found and built into the replay on 2026-10-10, against the eleventh and twelfth recordings, which have the figure in force on every frame). The length is the distance between two places in the world, the moved bone's and the muzzle's (or the farthest effect's), and at a reload's end those are taken while the squeeze of the frame before's drawing is still on the weapon's rig: everything along the view is at `s` of its size (0.664 at a field of view of 74). The code then divides by the muzzle's own scale along its barrel axis (`lossyScale.y`, if under 1), which the engine gives as how far that axis, squeezed, still reaches along itself: `1 - (1 - s) * c * c` for a barrel whose axis has a share `c` along the view. For a weapon lying along the view the two cancel exactly and the figure is the true length. For one that does not (canted to rack the bolt, carried across the body in a sprint, or just wandering a degree from the hip) they do not: what lies across the view is not squeezed and is divided all the same. So the figure depends on how the weapon lay to the view at that moment, and the reload from empty comes out 6 cm short not because the muzzle is 6 cm nearer but because the rifle is canted: true, the distance then is 0.859 m, and measured the game's way 0.832.

  Compared (the replay measuring so on the frame the reload's operation hands over, with the weapon's own bone as that frame's record has it and the bones above it as the frame before left them): over the 22 reload scenarios of the two recordings the figure is the game's within 0.8 mm on every frame in 20 of them, most within 0.2 mm, and within 1.1 to 1.3 mm in the two reloads from empty (it was 27 mm out there measured plainly, and 2 mm out for the reload in a sprint, which is now within 0.1). What is left is the weapon's random wander at that moment, which the squeeze makes the figure sensitive to: about a millimetre for each degree the barrel is off the view. `trp_sim::weapon_length_squeezed`; the replay calls it. **Not built in the app**, whose line keeps the weapon's length at rest. **Not worked out:** the figure as the weapon is first drawn (0.8973 and 0.8976 in the tenth and twelfth recordings against 0.8942 at rest, by this way of measuring too): the draw's last frames have the weapon's bone a few millimetres forward, and how far off the view the weapon still lies then is not on record before the first frame. **The ninth recording's depths fit a line of 0.9027 m**, 8.7 mm more than at rest ("Compared with the game" below); that is the draw's pose 0.05 s before its end, about one long frame before the hand-over. The simulation takes the length at rest, and a replay takes the recording's own where it has one: recorder 0.4.3 writes the game's figure into each trace's header (`weapon_length`), and 0.4.5 on every frame (`weapon_length_now`). **Not built:** measuring again at a reload's end, on the rig as the reload's clip has it. The tenth recording's eleven figures are the test for it.
- **The line.** It starts where the weapon's position spring has the moved bone (the spring's value, rest and all, as a point in the axes of the bone's *parent*), and runs along the barrel as that **parent** points, for the weapon's length. The parent (`Weapon_root`) is the bone the weapon's clips move. Nothing procedural turns it: not the sway, not recoil, not the lean, not a first-person pose (a roll lying down cants the weapon 70 degrees and the line still runs where the view points), and not the body carrying the weapon in a sprint, which moves the bone below it. (An earlier note here had the line follow the moved bone. It does not.)
- **The depth.** If the line hits something, the depth is the length less the distance to the hit. If it does not, a second line is cast from the player's own axis, at the height of the first line's start, out to that start; a hit there (the weapon's rear is already through something) counts as the whole length. Otherwise 0. Only the world's solid layers count (`WEAPON_OCCLUSION_LAYERS`), and never the player.
- **Passed on.** The depth goes to the effector as it is. From 0.02 up, the effector is also told how far back the position spring already holds the weapon (the spring's value along the barrel), which is taken off the room it has to draw the weapon back by. Below 0.02 blind fire is allowed, from there on it is not (`OverlappingAllowsBlindfire`, which refuses nothing in this build). In blind fire the line is first cast from its end point (`blind-fire.md`, "The overlap"; built 0.123.0).

**The sights** (`WeaponOverlapView`, right after, and `firing.md`, "Aiming, and what takes it away").

- With the depth over `STOP_AIMING_AT` (0.1 m, settings object) and the sights up, they are put down, and the controller remembers that it was the obstacle that did it.
- With the depth under that again, the effector's value under 0.2, that memory set and the sights down, they are put up again and the memory cleared. This is the one case in which the game raises the sights by itself.
- Any use of the aim key clears the memory: let go of the key while the weapon is pushed back and the sights stay down when it clears.
- While the depth is over the limit a press of the aim key is refused by the weapon at rest.
- Putting them down and up goes through the same call as the aim key, so what else refuses the key refuses this too: a state that does not allow aiming, a reload in hand below mastering 2, a look at the weapon. A raise that is refused is not tried again.

**How much** (`TurnAwayEffector.Process`, once per physics step, before the springs).

- The wanted value is the depth over a threshold: 0.42 m against the world (0.65 against another player; both on the player prefab). The value in use moves toward it by `lerp(value, wanted, 5 * dt)`, the same rate in and out.
- At a value of 0.001 or less the effector gives nothing at all.
- A second figure, the far blend, moves toward 1 while the value is over 1 and toward 0 otherwise, by `lerp(blend, target, 8 * dt)`.
- **It is switched off** by putting the depth to zero at the top of every physics step while the character crawls (the crawl's movement state clears a flag in the weapon motion's mask) and while any first-person pose is held (the roll lying down). The value then runs out at the rate above. The depth is measured all the same, and the sights still answer to it.

**What it does.** A pose of three position and three rotation components, each `curve(value) * lerp(near, far, blend)` with its own curve and its own two intensities. There is one pose for rifles, one for pistols and one for lying down (chosen by the prone flag, which is the weapon motion's stance 0). From the player prefab, first person, as (near; far):

| Pose | Position x, y, z (m) | Rotation x, y, z (degrees) |
|---|---|---|
| Rifle | (0.05; 0.14), (0.4; 0.07), (0.03; 0) | (0; 3.5), (-25; 0), (-3; -56.9) |
| Pistol | (0.05; 0.15), (0.4; 0.07), (0.03; 0.03) | (0; 16.9), (-25; -15), (-3; -60) |
| Lying down | (0.05; 0.05), (0.4; 0.4), (0.03; 0.03) | (-9; -9), (-25; -25), (-3; -3) |

In the weapon bone's axes x is to the right, y back along the barrel and z up: the weapon is drawn back by up to 0.4 m and a little right and up, and canted about its barrel by 25 degrees. The far figures are another pose altogether: for a rifle the muzzle swung 57 degrees aside about the weapon's upright axis, with the weapon hardly drawn back. The curves do not all start together: the draw-back begins at once and is nearly whole at a value of 0.9; the roll about the barrel only begins at a value of 0.5, the rifle's shift to the right at 0.3; all are whole at 0.95 and go on to about 1.3 at a value of 1.3, where they stay. With the depth at the whole length of a stock AKM the value settles at 2.1.

- The position along the barrel (y, which draws the weapon back) is multiplied by the field of view's squeeze and held between 0 and the pose's near intensity for y less what the position spring already holds it back.
- **Onto the weapon** (`AvoidObstacles`, every frame, last of all the procedural motion: after sway, recoil and the aiming alignment). The position is added to the moved bone's place in its parent. Then the bone is turned about the point 0.2 m ahead of it along its barrel, by the rotation taken as Euler angles of a vector that has first been turned from the bone's own axes into the world's. (So it is not quite a turn in the bone's own axes: the angles are applied about the world's axes in the engine's order z, x, y, and the result depends a little on which way the player faces.)
- **The camera.** From the hip its resting place is moved along the view by a figure that goes, by `lerp(shift, target, 8 * dt)`, toward a curve of the value (0 up to a value of 0.2, then down to -0.12 at 1) times how far the field of view's squeeze is from 0.65 toward 1; over a value of 1 the target is -0.03. Aiming, the eye's place along the view is moved back by the weapon's own draw-back, and its approach along the view is quickened by ten times that draw-back.
- **The elbows.** Two more poses (position only, and the same figures for both arms: x -0.3 near and 0.16 far, nothing in y or z, whole at a value of 0.85) shift the two elbow goals: each shift is turned by the view's yaw and 0.65 of its pitch and added to the weapon's own goal before the body's share is blended in (`AdjustElbows`). So both elbows are drawn up to 0.3 m to the left as the weapon comes back.

**Built** (2026-10-09). `Hands::weapon_line` gives the line in the rig's frame, `Movement::weapon_overlap` casts it and the second line into the host's world and returns the depth, and the host hands that to `Hands::set_overlap` and `Firearm::overlap` at the end of each frame. The world is asked through `World::line`; a host that says nothing gets the ground alone, from its height. The effector runs in the physics step, its pose goes onto the moved bone in `Hands::weapon_transform` and onto the camera with the rest, and `Hands::elbow_shift` is what a host adds to the weapon's goal for each elbow. The length is `trp_sim::weapon_length`, measured on the rig at rest (the game measures with the weapon in its resting clip). Not built: blind fire, the threshold against another player, the left-handed hold's own thresholds, and a mounted weapon, which does without the turn.

**Measured in the simulation** (stock AKM, standing still, a wall square across the view; `cargo test -p trp-sim --test sim a_wall -- --nocapture`). The line runs from 1 cm behind the ribcage to 0.884 m ahead of it, 0.959 m ahead of the feet.

| How far the line reaches into the wall | Value | What the turn-away adds | Sights |
|---|---|---|---|
| 6 cm | 0.14 | the weapon 7 cm back | stay up |
| 35 cm | 0.83 | 43 mm right, 365 back, 26 up; canted 21 degrees about the barrel; each elbow's goal 0.3 m aside | down on the frame the depth passes 0.1 m |
| 70 cm | 1.67 | the far pose: 185 mm right, 93 back; the muzzle swung 68 degrees to the left | down |

With room again the sights are back 0.28 s later, when the value has run down to 0.19. Lying down (the test `lying_the_ground`): level, nothing; with the view at its limit of 25 degrees down the line is 0.30 m into level ground, the value 0.72, and the lying pose adds 34 mm right, 370 back and 23 up and turns the weapon -7, -16 and -2 degrees; the sights go down as the view passes 18 degrees. In a crawl with the view there the depth is measured (0.37 m) and the value is nothing.

The character's own stand-off decides what this looks like against a wall. The game's capsule has a radius of 0.366 m and the engine's controller keeps a skin of 0.1 m outside it (player prefab, `CharacterControllerSpawner`), so the character's axis stops 0.47 m from a wall and the line reaches 0.49 m into it standing still (0.6 m while the legs still walk against it, the body leaning into the walk: the app's display). That is over the threshold: flat against a wall the far pose is what shows, and with it the swung weapon's muzzle is about 3 cm clear of the wall. (At the bare radius it would be about 7 cm inside it. That is taken as a sign that the skin counts; the capsule against a wall is not recorded.)

**Compared with the game.** Recorder 0.2.1 on writes the depth, the value and what the effector adds to the weapon, and the replay sets its own beside them, on the recordings' ground as a plane. On open ground there is next to none of it, so this is a check on the line and on nothing else:

- Sixth and seventh recordings (89 scenarios, 86,000 frames, the crawls, the rolls, the sprints with the weapon carried and the reloads among them): the game's depth is nothing on every frame, and so is the simulation's.
- Fifth recording, the turn lying down (28, view 12.2 degrees down): the game has a depth on three frames, 18.2, 13.6 and 5.5 mm of the line, which at that pitch is its far end 3.9, 2.9 and 1.2 mm under the ground. The simulation's line has its lowest point on the same frame and is 0.9, 1.6 and 3.3 mm over the ground on those three: 4.6 mm higher at the end of a 0.9 m line, each time. Whether that is the ground under the weapon (the replay's is a plane fitted to the walks), the line's length (22 mm more would do it) or its angle (0.3 degrees) is not settled. The game's value reaches 0.0044 there; the simulation's stays at nothing.

Scenarios 53 and 54 of recorder 0.4.1 were written for more (lying and crouched, the view pitched down with the sights up) and were recorded (eighth recording), but their turns were sized without the aiming sensitivity and stop short: lying the view gets 12.2 degrees down and crouched 44.9, and the weapon's line stays 12 cm and 17 cm clear of the ground. They settle nothing about the push-back.

- **Ninth recording (2026-10-09, recorder 0.4.2: 53 and 54 alone, rewritten to turn the view to its limit).** The weapon meets the ground in both: the game has a depth on 588 frames lying (up to 0.140 m, the value up to 0.30) and on 321 crouched (0.204 m, 0.48). This is the push-back recorded for the first time, both its poses, and the sights with it.

  | | 53, lying | 54, crouched |
  |---|---|---|
  | View when the line first touches | 21.3 degrees down | 59.5 |
  | Depth, simulation against game, all frames | 0.1 mm rms, 2.1 at worst | 0.1 mm rms, 0.2 at worst |
  | The value | within 0.0015 | within 0.0005 |
  | What the pose adds to the weapon | within 0.6 mm and 0.014 degrees | within 0.1 mm and 0.003 degrees |
  | The weapon's moved bone, in the player's axes | 1.0 mm rms, 3.3 at worst | 1.0 mm rms, 2.2 at worst |
  | Sights put down by the obstacle | on the game's frame (7.02 s, depth 0.1002 m) | on the game's frame (5.09 s) |
  | Sights given back | two frames early (the value 0.1999 where the game's is 0.2008) | on the game's frame (7.48 s) |
  | Field of view | the game's but for those two frames | the game's on every frame |
  | Camera beside the weapon | 1.9 mm rms | 0.8 mm rms, 7 at worst |

  Read plainly: the rule for the sights is the game's (down past 0.1 m; back by themselves when the depth is under it and the value has run down under 0.2), the value follows the depth at the rate read, and both poses and their curves are right. **Two figures behind these numbers are fitted, not read:** the line's length (0.9027 m; above, "When it is measured") and how far under the feet the ground was (7.9 cm; this recording has no jump and nothing resting on the ground to tell it by). They were chosen from the depths themselves, two numbers against 909 frames with the view anywhere from 21 to 90 degrees down; with the length at rest and the ground as on the eighth recording's spot the depth is 4 to 7 mm short throughout and the sights change two frames off. Recorder 0.4.3 records both, so the next recording needs no fitting.
  - **What it did not test.** The second time lying, the view had come up past level first, so the same turn down ended at 23.4 degrees with the weapon 7 to 9 cm in: under the limit, the sights stayed up, and the aim key let go and pressed there raised them again as it would anywhere. Whether sights the obstacle put down stay down once the key has been used was still from the code; the tenth recording has it (next).
- **Tenth recording (2026-10-09, recorder 0.4.4: all 56 scenarios; 53 with its first turn up shortened so that the second turn down reaches the view's limit).** Nothing fitted this time: the line's length is the game's own from each trace's header (0.9033 m for both scenarios, which follow a reload) and the ground under the feet is the recorder's own ray (0.0070 m on this spot; the standing jump of scenario 08 lands 0.0070 m lower, which agrees to three thousandths of a millimetre).

  | | Lying (53, 3,115 frames) | Crouched (54, 1,559 frames) |
  |---|---|---|
  | Depth, largest gap | 0.2 mm | under 0.05 mm |
  | The value, largest gap | 0.0019 | under 0.00005 |
  | What it adds to the weapon | 0.4 mm, 0.12 degrees | under 0.05 mm, 0.001 degrees |
  | Sights | the game's on every frame: down by the ground at 4.85 s, back by themselves at 9.20 s, down again at 16.24 s and down from then on | the game's on every frame: down at 4.58 s, back at 7.62 s, down by the key at 11.00 s |
  | Field of view | the game's on every frame | the game's on every frame |

  - **Sights the obstacle put down stay down once the key has been used.** In 53 the second pass puts the sights down with the weapon past the limit; the aim key is let go at 18.0 s and pressed again at 18.5 s with the weapon still pushed back; the view then comes up and the weapon clear, with the key held, and the sights do not come back, to the end of the scenario. The first pass, with the key held throughout, gives them back. As read and built, on every frame.
  - **The ninth recording's two fitted figures.** Its line, 0.9027 m, is in the range the game measures after a reload (0.9014 to 0.9043 in the tenth; above, "When it is measured"), not what it measures as the weapon is first drawn in a raid (0.8973 in the tenth), so the weapon had been reloaded, or drawn on another frame, before that recording; nothing in the ninth says which. Its ground, 7.9 cm under the feet, is a figure of its own spot: the tenth's spot had 0.7 cm, the eighth's 7.6 cm, all within the reach of the ground check that lets a character ride over the ground.
  - **One frame of the camera** is not the game's: on the frame the aim key is let go with the weapon pushed back, the simulation's camera goes 5 cm further along the view than the game's (9.4 cm against 4.5), and the two are together again on the next. Not looked into.
  - **Found on the way, in the replay:** the breath (the weapon rises and falls up to 7 mm with it, and the line starts at the weapon) and the physics steps (which frames the springs move on) are now put where each recording has them, and with that the weapon's place is nearer the game's in every recording: 2.6 to 3.3 mm rms on average where it was 4.4 to 5.8.
- **The obstacle's change of the sights reaches the weapon's motion at once.** It is made at the end of a frame, after that frame's camera and physics steps. The game's weapon motion is told then, and the field of view's easing, which is a routine that takes its first turn the moment it is started (a turn that changes nothing yet), has its first visible step on the next frame. The simulation left it to the next physics step, a frame or two later. Built: `Hands::aim_changed_after_frame`, which a host calls after `Firearm::overlap`.

- **The ground the line meets is the terrain, not where the feet are.** The character rides as much as 8 cm over the ground, and the game's line is cast at the ground itself. On the eighth recording's spot that is 7.6 cm lower ("The camera's collider" above says how that is known), which lying down is the difference between the sights coming down with the view 18 degrees down and 24 (recorded in the ninth: 24.3, on ground 7.9 cm under the feet; the view's limit is 25). The replay casts at the terrain where a recording's folder says how far under the feet it is; the fifth recording's three frames of contact above were compared with the ground taken at the feet, which may be what its 4.6 mm is.

**It is off in a crawl.** Entering the crawl clears a flag in the weapon motion's mask, and with it cleared the depth is put to zero every step. The fifth recording has the value and what it adds at zero on every frame of the crawl.

## First-person poses of body states (`AnimatorPose`)

A state of the body's animator can name a pose for the first-person weapon: an offset in position and rotation for the weapon, the same for the camera, and a curve of time. Only the rolls to the side lying down carry one, on their way in and their hold: for the right, position (-0.1, 0.03, -0.015), rotation (0, 35, 0), camera rotation (0, 0, 5); for the left, position (-0.02, 0.04, -0.02), rotation (0, -35, 0), camera rotation (0, 0, -5); the curve runs from 0 to 1 over 0.4 s and holds to 0.6.

- **Which poses are live.** Every time the animator enters a base-layer state that has the game's script, the weapon motion is told that state's pose, or that it has none. Every pose it holds that is not that one is marked as going out; that one is marked as coming in, and added with its clock at 0 if it was not held.
- **Each frame**, after the rest values are set from the lean and before the weapon bone is placed, each held pose's clock moves by the frame's time times the weapon's flip speed, forward if coming in and back if going out. A clock that falls below 0 drops the pose. Otherwise the curve's value at the clock, times the pose's position and rotation, is added to the rest of the weapon's position spring and of its rotation spring, and the clock is held to the curve's length. (The game multiplies the time by the flip speed once more for every further pose it holds, working from the newest: with two held, the older runs at the flip speed squared.) The frame's time is the one the camera gets: the physics time on a frame with a physics step, the frame's own otherwise.
- **The camera.** When the camera's target is worked out, each held pose's camera rotation times its curve's value is added to the rest of the camera's rotation spring. The pose's camera position would be added to the camera's place from the hip, but only under a figure that nothing ever sets: like the sprint's zoom, it is dead.
- **A rest counts twice.** The springs are drawn to their rest and the rest is added again when they are read ("Springs" above). So the roll's 35 degrees turn the weapon 70 once the spring has caught up, and the 5 of camera roll are 10. Recorded (sixth recording, 38): the weapon bone is turned 70.1 degrees from where the simulation had it before this was built.
- **The flip speed** is `Inertia.WeaponFlipSpeed` of `globals.json` evaluated at the character's inertia, set whenever the carried weight changes: 1 to 0.5 in this install, 0.971 for the recorded character. The value on the player prefab (1) is only where it starts.
- While any pose is held the turn-away from obstacles is switched off, and so is the aiming flag of an effector not covered here.

`trp-data` reads the poses (`MovementBehaviour::first_person_pose`; `probe bodystates` prints them). `Movement::take_pose` hands a host each change as the animator enters states; `Hands::assign_pose` and `Hands::blend_poses` are the two steps above.

## Compared with the running game

From traces recorded with the plugin in `hosts/spt-oracle`, stock AKM, standing:

- One shot from the hip: the game's muzzle climbed 1.80 degrees and the simulation's 1.61, with the same shape step for step and the same return through zero 0.4 s later; the weapon came back 17 mm against 19, the camera 0.79 degrees against 0.71. The differences are within the shot's random strength (0.9 to 1.15).
- A one-second burst: both fire 10 rounds. The climb per shot, the hold from the fifth shot (the game between 5.7 and 8.4 degrees, the simulation between 6.4 and 7.2 in that run) and the return with its overshoot of about 1 degree agree.
- A full lean: both springs settle at 5 and the camera's roll at 10 degrees.
- Breathing: 0.5 degrees and 10 mm per breath in both.
- Turning (second recording): the sway is within 0.13 degrees of the game's once the replay turns as the game did and the sway spring takes its settings from the settings object. The scripted turn rate is multiplied by the mouse sensitivity in the game (0.70 here), which the first comparison missed.
- Turning (eleventh recording, 2026-10-10, and every recording before it replayed again): that 0.13 degrees was not noise. The sway takes the view's turn a frame late ("Motion reaction": the rig is turned to the view after the frame's physics steps), and the weapon's weight it scales with is the one the game last took stock of ("Sway factors"). With both, and with the replay working its physics steps on the frames the recording shows a spring moving on, the sway is the game's within 0.0013 degrees on every frame of every scenario of the eleventh recording and within 0.0012 in the fourth, sixth, seventh, eighth and tenth, where it had been 0.03 to 0.29 out in every scenario that turns and 0.83 in a flick at 420 degrees a second. The weapon's own gap from the game's hardly moves for it (0.75 degrees and 2.6 mm root mean square over a recording, as before): that is the breath's and the hip wander's random numbers, as it was.
- Aiming (second recording): the eye's aimed position agrees to 0.2 mm sideways and 0.1 mm forward, the approach has the same alternating rate, and the field of view eases the same way. The recorded character brings the sights up in 0.87 s against 1.16 s for an untrained one: skills add about a third to the aiming speed.
- The weapon bone's own rotation and position now compare (sampled at draw time): within about 1 degree and 6 mm, which is the size of breathing and the hip wander, both random.
- Not comparable in the replay: what the clips do to the camera (a fire-mode switch turns it 2.5 degrees at full head bobbing), since the replay plays no weapon clips.
- Third recording: the field of view stays at the setting through a sprint (the sprint zoom was never in the game); the squeeze at a wide field of view is applied once; and the weapon carried by the body in a sprint agrees as given under "Carried by the body".
- With the body's animator running, what depends on walking compares too. The weapon bone as the camera sees it is within 3 to 9 mm in every scenario on open ground; its rotation within 0.2 to 1.3 degrees root mean square (2.5 degrees at worst outside the jumps); the camera's rotation within 0.3 degrees, and 0.7 in automatic fire, where the kicks are random.
- Fourth recording (28 scenarios). The weapon bone is within 1 to 9 mm and 0.3 to 1.9 degrees root mean square in 26 of them, the jumps and the turns on the spot now among them (a standing jump 6 mm and 0.5 degrees, where the landing's timing had left 9.5 degrees at worst).
- **The weapon in a crawl** (fourth and fifth recordings). The game's was turned a constant 22.8 degrees from the simulation's in the weapon bone's own axes, and 9 cm away, whichever way the crawl went. The cause was the hand's hold ("In the body's hand" above): with it the two agree as given under "Carried by the body". The turn-away, suspected first, was ruled out by recording it.
- Fifth recording (the same 28 scenarios, the other way along the same ground): the weapon bone is within 3 to 8 mm and 0.3 to 1.8 degrees root mean square in every scenario, the four lying down among them.
- Shots fall on the game's frames once the weapon's logic is stepped once a drawn frame (`firing.md`): the round count is the same on every frame of the four firing scenarios.
- Seventh recording (49 scenarios, a heavier pack): the weapon bone is within 1.2 to 9.9 mm and 0.2 to 1.8 degrees root mean square in every scenario, 20 mm and 4.2 degrees at worst. New in it: the weapon through a reload in a sprint (2.5 mm and 0.6 degrees, 6 and 2.2 at worst) and a look at the magazine in one (6.1 and 0.8; 12 and 1.3), where the body lets two thirds of the weapon go until the hands are done (`body-animator.md`, "Reloading"); and the aiming eye with the sights brought up in a reload's tail, which rides on the weapon as the reload's clip still has it (`firing.md`).
- The replay now plays the weapon's clips on a rig of its own (`trp-sim/src/rig.rs`), for the arms. What the clips do to the camera is still not fed from it.
- **The stance lying down** (2026-10-09). The weapon motion's stance is 0 for a character whose prone flag is up, whatever the stance level, and breathing and recoil take their factor for it (0.6 and 0.6 to 0.7 against a crouch's 0.7). The simulation had never told the weapon motion that the character lay down, so lying it breathed and kicked as crouched. With it told (`HandsInput::prone`), the weapon's gap in the seventh recording's twelve scenarios that lie down is smaller in nine and the same in three (root mean square: the plain lying one from 6.2 to 5.7 mm, the roll from 4.0 to 3.4, the prone key in a reload from 1.7 to 1.3), and the other thirty-seven scenarios are unchanged to the last figure. The turn-away takes its lying pose by the same flag.
- **Recoil, shot by shot** (2026-10-09; the user asked for the recoil to be gone over "for peace of mind"). The first comparison above had one shot and one burst. The recordings now hold 179 shots between them (the third to the tenth: single shots from the hip, aimed and lying, and a one-second burst from the hip and aimed in each), and the replay takes a seed (`replay --seed`), so the game's shots are set against the simulation's spread: forty seeds on the tenth recording's frames and conditions, 1,040 shots. The game's random draws are its own, so it is sizes and ranges that compare, not one shot with another.
  - *Each shot's own force, worked back from the springs.* The position spring and the camera spring step as `V = (V + F - C * r) * d; C += V`, so the force a step took is `(C1 - C0) / d - (C0 - Cm1) + r * C0` from three recorded values (`local\kicks.sh`). Standing, 169 shots in the game: the kick back is 18.48 to 29.72 mm, mean 23.7 (the simulation: 18.49 to 29.78, mean 24.1); the kick to the camera 0.795 to 1.014 degrees, mean 0.900 (0.795 to 1.014, mean 0.906); the kick's direction before steady fire 80.1 to 89.9 degrees from level, mean 84.9 (80.0 to 90.0, mean 85.4), and from the fifth shot of a burst, where its range widens, 70.3 to 90, mean 82.7 (66.3 to 90, mean 81.8). Lying, 10 shots: back 15.3 to 20.8 mm (12.9 to 20.8: the top is the standing top times the lying factor of 0.7 on both sides; the game's ten happen not to reach the bottom, a one in thirty chance), camera 0.481 to 0.603 degrees (0.478 to 0.609). One recorded shot fell on a frame with two physics steps, which this working-back misreads; it is left out.
  - *A single shot, as the springs answer it* (`local\shots.sh`: the largest value after each shot). From the hip, 14 in the game: the muzzle climbs 1.82 degrees on average, 1.62 to 1.99 (the simulation 1.77, 1.56 to 1.99), reaching it 0.143 s after the rounds drop (0.141); the weapon comes back 18.0 mm, 15.5 to 21.7 (19.8, 15.3 to 24.5); the camera kicks 0.81 degrees (0.79); half a second on the muzzle is 0.15 degrees past rest on its way back (0.15). Aimed, 14: 1.33 degrees, 1.17 to 1.48 (1.34, 1.17 to 1.51), 18.6 mm (19.8), camera 0.80 (0.80). Lying and aimed, 8: 0.80 degrees (0.80), 15.4 mm (13.9), camera 0.48 (0.48). Every recorded figure lies inside the simulation's own least and most, or within half a percent of them.
  - *Automatic fire, the muzzle's height as each shot fires* (7 bursts each way in the game, 40 in the simulation; degrees). From the hip, shots 2 to 5: 1.65, 2.57, 3.66, 5.19 in the game and 1.67, 2.62, 3.73, 5.27 here; held from the seventh at 7.3 on average, between 6.0 and 8.8 (7.1, between 5.3 and 9.5). Aimed: 1.20, 1.92, 2.74, 3.86 and 1.16, 1.92, 2.74, 3.85; held at 5.6, between 4.9 and 7.1 (5.3, between 4.0 and 7.1). The game's first four are 1.5 to 2% under the simulation's from the hip and level with them aimed; with seven bursts that is inside the spread.
  - *What was wrong: when a single shot's kick reaches the springs.* Found by the time to the muzzle's peak, which was 0.143 s in the game and 0.133 here. The game does not kick the weapon when the shot is made: the shot raises a flag, and the firearm controller's late update, after the frame's physics steps have gone through the springs, gives the recoil its kick; and a single shot takes its round a frame before it makes the shot (`firing.md`, "The recoil's kick, and which physics step takes it"). With that built, the frames from the rounds dropping to the springs first moving are the game's for all 16 recorded first single shots (they were the game's for 8), and the time to the peak is 0.141 s.
  - *Not compared:* the weapon's own recoil curves and the breath's answer to a shot, which do not show in the three springs' values; the return's sideways path beyond the figures above; and anything of other weapons, rounds or stances between standing and lying.

- **Tired breathing, against all ten kept recordings (2026-10-10).** The replay now gives the breath the hands' and the oxygen's levels from the simulation's own stamina, which is the game's on those recordings. Nothing measurable came of it: the movement states, the distances and the eye are the same to the last figure in every scenario, and the weapon's gap from the game's moved by a hundredth of a millimetre on average in either direction (2.42 to 2.43 mm in the twelfth, 2.59 to 2.58 in the seventh). The pools on record never go low enough: the least the legs hold in any recording is 0.87 of their capacity and the arms 0.9, where the curves the breath reads have hardly moved from their values at 1, and what the weapon is out by is the breath's and the wander's random numbers. A recording that runs a pool down (`61-legs-dry` ends on empty legs with the sights down; one that aims for a minute would spend the arms) or holds a breath is what would test it.

- **The eye on the sights' line (2026-10-12; "The sights").** The replay reads the sights off its own rig as the app does, so the sixteen kept recordings compare it (`local/regress.sh after34` against `after33`, the run before the rule; `after35` since). No state of any scenario changed and no distance moved. The eye's largest gap from the game's shrank in 26 scenarios and grew in 13, each by tenths of a millimetre, except the M3's: in the sixteenth recording's `76-m3-pump-aimed` the aimed eye was 14.9 mm from the game's at worst and 12.2 mm root mean square, and is 1.5 mm and 0.6 mm; in `73-m3-reload-pump-aimed` 2.8 mm root mean square became 0.7 (its worst, 12 mm in the first frames of the sights coming up, stays). The AKM's aimed scenarios went from 1.3 to 0.9 mm root mean square (the thirteenth's `14-aim-and-walk`) and from 0.9 to 0.7 (the seventh's `35-aim-going-prone`). So the AKM had hidden the fault: its rear sight's bone is a millimetre from its own. The Five-seveN and the Model 700 are on no recording.

- **Held aimed fire through a scope (2026-10-24, 0.116.2; "Camera", "In automatic fire"; "What a shot does").** Course 6's M60E6 (`r18`), the eye's place and line set in the frame of `Weapon_root_anim`, frame for frame (`local\eye_in_weapon_s124.pl`). With the game's own recoil put on the replay (`replay --recorded-recoil`, so that the random draws are the game's): the line's pitch 0.048 degrees rms; the eye's height against the scope 0.22 mm rms, 0.8 at most (1.75 and 4.7 before the automatic fire's target); the camera's place 0.2 mm rms, 0.7 at most; its turn 0.017 degrees rms (0.055 before the shot was felt after the pose). With the replay's own recoil the eye's height is 0.84 mm rms apart (4.7 at most): the draws. What remains with the game's recoil is the hands' slow breathing wander (0.1 to 0.2 degrees of the weapon's turn, apart before the first shot).

## Not covered yet

Holding breath and tiredness, force reaction (hits, grenades), blind fire (since built: `blind-fire.md`), hand shake, the stiffening of the weapon in a reload with the sights up (read in `firing.md`, not built), the aim planes and the forward camera position when aiming through an optic, aim sway, optics and the choice between several sights, the shot's direction for iron sights (with ballistics), mounting and bipods, overheating, and what skills change besides the animator's two speeds (`firing.md`, "The skill's two speeds").
