# The left shoulder: the weapon carried on the left side

The user's order (2026-10-24, again 2026-10-25: "Let's work on the left shoulder then getting all of the durability, and weapon malfunction systems in"). Read from the decompiled code in session 127 (`local\blind_mount_s127.md`, section 3) and again in session 133; recorded by recorder 0.13.0 (`courses6`, course 8, step `04-left-shoulder`, the MDR and the M60E6 twice). The game's names: `EGameKey.LeftStance`, `ECommand.LeftStanceToggle`, `LeftStanceController`, `MovementContext.LeftStanceEnabled`, `PlayerBones.ShiftWeaponRoot`.

## The key

`LeftStance` (the install's `Mouse4`, pressed; the press type is fixed: one press flips) gives `LeftStanceToggle`. Only the firearm's input translator hears it: a knife, a grenade or a medkit in hand ignore it. `FirearmController.ChangeLeftStance` refuses it in blind fire and mounted; otherwise the left hand's item is put away and the operation in hand is asked (`ToggleLeftStance`): only the weapon's rest (`Idling`, and so the looks, `UtilityOperation`, built on it) and a launcher's rest pass it on; every other operation drops it (a reload, a shot, a draw, a malfunction). The rest refuses a stationary weapon, a weapon that blocks it (`BlockLeftStance`, false for every weapon of the install), the compass out, and lying down (`IsInPronePose`). Passed on, the controller flips.

## The controller (`LeftStanceController`)

A state machine with no timer. **The stance wanted** (`LeftStance`) is flipped by the key and kept through everything that takes its animation off; `LastAnimValue` is what the controller last gave the animator by its own rules. Its writes go to the body's animator (`MovementContext.SetAnimatorLeftStance`: the body's `LeftStance`, and with it off `ForceLeftStanceInOneFrame` off; `SetAnimatorLeftStanceInOneFrame`: both on) and to the weapon's motion (`ProceduralWeaponAnimation.LeftStance`, whose only reader, the turn-away's `LeftStance`, is never read).

- **The key** (`ToggleLeftStance`): the wish flips; the animator follows unless a body action holds it (`_blockChangeToLeftStanceFromBodyAction`) or the body may not change it (`_allowBodyChangeAnim` false, as it is from the start until the hands first put it back).
- **A body action takes it off** (`DisableLeftStanceAnimFromBodyAction`): at once where the body may, else it is asked for later (`DisableAnimLeftStance`); either way the body holds it.
- **A hands action takes it off** (`DisableLeftStanceAnimFromHandsAction`): at once, the body may no longer change it, and a flag is set that nothing ever clears (`_blockingLeftStanceOperationInProcess`).
- **The inventory opened takes it off** (`DisableLeftStanceAnimFromOpenInventory`): the same, without that flag.
- **A body action is over** (`SetAnimatorLeftStanceToCacheFromBodyAction`, its argument "in one frame"): nothing without a firearm in hand (`HasFirearmInHands`); where the body may, the animator back to the wish (in one frame out of a sprint, where the wish is on), else asked for later (`SetToCacheValue`); the body's hold let go, and the body may change it again if the hands let it meanwhile.
- **The hands put it back** (`SetAnimatorLeftStanceToCacheFromHandsAction`): nothing without a firearm; a body action's taking off asked meanwhile is done now (unless the body still holds it), else the wish, unless the body holds it; then the body may change it, at once or when its hold ends.
- **The inventory shut** (`SetAnimatorLeftStanceToCacheFromCloseInventory`): as the hands' putting back, but never once a hands action has taken it off (the flag never cleared: after the first reload the inventory's shutting puts nothing back, and the next rest does).

## What takes it off, and what puts it back

**The hands** (`SetLeftStanceAnimOnStartOperation`, called by every hands operation's start): the weapon's rest puts it back (`Idling`: not with a stationary weapon, the inventory open, or a wall having taken it off), at its start and at its rest state's `IdleStart` mark (`Idling.OnIdleStartEvent`); a launcher's rest puts it back; the weapon's draw puts it back (`SpawnOperation`; for a weapon that blocks it, `SetLeftStanceForce(false)`); the fire of every kind (`FireOperationBase` and what is built on it) and the malfunctions (`MalfunctionOperation`) leave it; a launcher toggled leaves it; every other operation of the weapon takes it off (`FirearmOperation`'s, which a reload, a look (`UtilityOperation`), a fold, a magazine moved from the inventory and the put-away keep), and so does every other thing in hand's draw (a grenade's, a knife's, a medkit's, an item's) and the quick throw and the quick knife. The weapon's left hand's own gestures take it off from the hands (`Interact`, `Pickup`, `ShowGesture`), and the left hand's layer falling from whole puts it back (`ProcessLeftStanceResetAfterLeftHandInvoke`). **The inventory**: `FirearmController.SetInventoryOpened`, open and shut, before the operation hears it.

**The body's states** (each state object's `Enter` and `Exit`; a state built on another calls its base's first):
- The rest (`IdlePlayerState.Enter`) puts it back from the body, and with it every state built on the rest: blind fire's, the step aside, the way down to lying (which then takes it off), the way up (which puts it back again).
- The walk (`MovePlayerState.Enter`) puts it back, neither sprinting nor lying down; the crawl is built on the walk and so does nothing.
- The sprint takes it off as it begins (after the walk's entry, which the sprint makes do nothing) and puts it back **in one frame** as it ends (`ForceLeftStanceInOneFrame`: the `LS` layer plays its stance in from the sprint at once).
- The jump takes it off as it begins and puts it back as it ends.
- Lying still (`PronePlayerState`) takes it off as it begins, and puts it back as it ends off the ground; the roll is built on it (its way in takes it off twice; its end, off the ground, puts it back twice; half way out it puts it back once, off the ground).
- A vault over something higher than a hand's free height (`VaultingSettings.Vault.MaxWithoutHandHeight`) and a climb at least as high (`Climb.MaxWithoutHandHeight`) take it off as they begin; their end puts it back (`VaultingMovementState.Exit`).
- The kick's approach takes it off on every frame of it.
- An animated interaction of the body (`AnimatedInteractions.IsInteractionPlaying`: a door, a gesture on the world) takes it off on every frame it plays, and its end puts it back (`Player.VisualPass`).
- **A mount turns it off for good** (`IdleWeaponMountingState.Enter`: the wish flipped off).
- A stationary weapon takes it off and its end puts it back.

**A wall** (`FirearmController.WeaponOverlapping`, at the frame's end): the weapon is felt along its line from the hands' place in the weapon root's own axes while the stance is wanted (in its parent's otherwise), along the weapon root's way. With the stance wanted, not lying down, the inventory shut and the wall's wait over (`float_7`, which runs a second, `float_6` 1, from naught at each of its changes and at a new weapon, from -1 as the inventory shuts): a weapon meeting something at the full curve takes it off from the hands and remembers so (`_disableLeftStanceByOverlap`); once the animation is all the way back (the curve at 0, `LastAnimValue` off) and the plain line clear, a line from 0.2 m further left (`-WeaponRoot.right`) and as much longer is tried, and clear, the hands put it back.

**Blind fire**: a start is dropped while the stance is wanted (`Player.ToggleBlindFire`, before the state hears it); its end is not.

## The body

**Layer 14, `LS`** (additive, weight 1; `probe bodystates LS Player_anim_controller`, `local\body_ls_states_s133.txt`): from `New State` to `LS_idle_aim_0` (`LS_idle_aim_0_additive`, 0.5 s, blend a tenth of the state) on `LeftStance` and not forced, or to `LS idle aim from sprint` (speed 0.8, then into `LS_idle_aim_0` at its end) forced; out by `LS_idle_aim_0 0` (`LS_idle_aim_0_exit` at **speed -1**, blend 0.05 s) on `!LeftStance`, back to `New State` after 0.9 of it. Its clips ramp `Leftstance_curve`, `First_Person_Curve_Weight`, `Shoulder_Left` and `Elbow_Left` from 0 to 1. **A state played backwards** (recorded: the exit's normalized time ran 0.1 to 0.9 over 0.4 s as the curve went 0.9 to 0.1): its time runs on as any state's, its clip taken from its end. Layer 9, `Additive_ISaim`, picks its `_LS` aim clips by the same. `LeftStanceFloat` has neither writer nor reader (recorded 0 always). The weapon's animator has no left-stance parameter; there is no sound.

## The weapon (`PlayerBones.ShiftWeaponRoot`, in first person)

Each frame, after the body (the IK pass), the weapon's moved root (`Weapon_Root_Anim`) is put by the body's own weapon bone (`Weapon_Root_Third`) with the procedural motion's own place and turn (`PlayerBones.Offset`, `DeltaRotation`, which the weapon motion's pass leaves in the root's local place and turn):
- **At the full curve, out of a sprint**: its place is the body's bone's with that offset on it, at once, or eased after it by the body's frame time times 10 (`leftStanceRotationAndPositionChangeSpeed`, the player prefab's) while aimed and not walked forward since the stance came in full (`_wasMovingInLsAim`, the forward key). Not aimed, its turn is the body's bone's with `DeltaRotation`, at once on the first full frame, else eased from the last frame's by the frame time times 10, or 20 while the animation is going back (`leftStanceReturnBackSpeed`); turned only about its own y (its x and z angles kept as the procedural motion has them). **Aimed**, its place in its parent is held at the ribcage's own place in its parent plus (0.13, -0.305, 0.023) times 0.2 (the code's `_offsterDirrectionForLeftStanceInAim`, `_offsetForRootAimInLeftStance`), eased by the frame time, the recoil's hand push added; its turn eased from the last frame's toward the procedural motion's own by the frame time times 10, turned only about y.
- **Below the full curve**: toward the body's bone by the curve or the body's own authority (`Weapon_3rd`), whichever is more, turned only about y while the curve is up; at no curve, by the authority alone (as before the stance: the sprint and the crawl).
- Seen from outside it is the body's bone by its authority alone.

## The camera

While the curve is up (and blind fire's blender at 0) the camera's pass runs only late in the frame (`ProcessEffectors` leaves it to `LateTransformations`), by the frame's own time, after the shoulder has moved the weapon: its target is the sight on the weapon where the shoulder put it, and the aim plane is taken in the camera's parent's axes. Aimed with the curve up, the eye's target comes nearer by 0.06 times one less the field of view's squeeze (`_aimLeftStanceAdditionalOffset` times `1 - _compensatoryScale`: nothing at a field of view of 50 or less). A canted sight's own turn in the stance (`LeftStanceRotation`) is worked out and never read; the neck's and the head's left-stance turns blend at nothing.

## Third person

The packet sends `LastAnimValue`; an observer's controller writes only `LeftStance` (never the force) and the body plays layers 14 and 9 as above; the weapon is the body's.

## Built

(2026-10-25, 0.127.0.)
- **The controller** (`trp_sim::left_stance::LeftStance`, `HandsStart`, `Call`), the game's methods one for one.
- **The movement** (`movement/left_stance.rs`): the key (`Input::left_stance`, `left_stance_may`), the refusals (blind fire, mounted, lying down, no firearm), the body states' entries and exits, the approach's frames, the body's animated interaction, the mount's turning it off, blind fire's start dropped, the wall (`left_stance_overlap`, its wait, the inventory's two seconds); `left_stance_call` for the hands; the body's `LeftStance` and `ForceLeftStanceInOneFrame`.
- **The weapon's logic** (`trp_sim::Firearm`): `Event::LeftStance` at every change of the operation's kind (`left_stance_start`: the rest, the draw, the fire left alone, every other off), at the rest's `IdleStart`, and as the inventory opens and shuts; `may_toggle_left_stance` (the rest and the looks).
- **The body** (`trp_sim::Body`): the `LS` layer's clips read; `left_stance_curve`, `ribcage_local`, `weapon_root_third` (the body's weapon bone whatever its authority); the animator's states of negative speed played backwards (`animator::Share::reversed`).
- **The weapon's place and the camera** (`trp_sim::Hands`): `weapon_transform_shifted` (`LeftShoulder`, the prefab's speeds, the aim's offset; `Shifted`: as carried and as held by the right palm); `shoulder_line`, `measure_overlap_shoulder`; the camera's pass by the frame's time with the curve up, from the weapon as the shoulder placed it (the app and the replay); the eye's nearer target.
- **The switch** `LEFT SHOULDER` under `SIMPLE`, after `MOUNTING` (`trp_sim::Systems::left_shoulder`: off, the key is not heard); the binding `LeftStance` among those acted on (with the mount's, the bipod's and blind fire's, which were missing from that list); `--tap` knows `mouse3` and `mouse4`.
- **The replay**: `cmd:LeftStanceToggle`, the hands' calls, the wall, `p_LeftStance`, `p_ForceLeftStanceInOneFrame`, `p_Leftstance_curve`.
- **Tests**: `left_stance::tests` (two), `tests/sim/left_stance.rs` (two: the body's curve in and out, the drops and returns).

## Compared with

**Course 8 of `courses6`, `04-left-shoulder`** (recorder 0.13.0; the script: the stance at 1 s, aimed 2 to 7 s with two shots, aimed again 9 to 12 s walking 10 to 11.5 s, a walk into a sprint 13 to 15 s, a reload at 15.5 s, down and up at 17 and 18.5 s, the stance off at 19.5 s), replayed from the course's scripts with the recorded builds (`local\ls_run_s133.sh`; reports `local\ls_replay_*_s133.txt`):
- **The MDR**, 2308 frames: the animator's `LeftStance` and the body's `Leftstance_curve` **gap 0 on every frame** (the stance in, the sprint off and back in one frame, the reload off and back, lying down off and the key's wish kept, the exit by the clip played backwards); the movement state, the pose and the hands' animator gap 0; the weapon root's place within 3.8 mm (rms 1.3 mm) and its turn within 1.4 degrees (rms 0.7, about the 0.5 to 1.1 degrees the replay has on this weapon at rest before the stance comes in; aimed in the stance it is 0 to 0.3); the camera within 1.8 mm (rms 0.5 mm), the eye 2.3 mm. Before the camera's pass was moved late by the frame's time it led the game's on the way into the aim by 1.7 cm (3.3 cm at most).
- **The M60E6**, first run: the same until the movement parts at 11.67 s (the walk's and the sprint's frames: the heavy weapon's movement, not the stance's), the stance's flag then a frame or two apart at the sprint (rms 0.02); its hands' animator and left palm differ from the first frame, before any stance. Second run: it walks into the low wall, which the replay's flat ground does not have, and parts from 10.3 s.

**Looked at** (`local\shots\`): `ls_rest_s133.png` (the AKM on the left of the view at rest), `ls_aim_s133.png` (aimed: the sights before the eye, the left arm in view). From outside the watching camera always looks at the character's one side: not judged.

**Read from code only, not recorded**: the wall's drop and return; the jump's, the roll's, the vault's and the climb's parts; a mount's turning it off; blind fire's refusal; the inventory's; an animated interaction's; the left hand's gestures; a pistol (recorder 0.13.0's course 8 has it for the MDR and the M60E6 only); third person.

## Not built, or not read

- The left hand's own gestures and layer (`Interact`, `Pickup`, `ShowGesture`, `ProcessLeftStanceResetAfterLeftHandInvoke`): the weapon's left hand's layer is not driven here; the body's interaction stands for them.
- A launcher's rest, the compass, a stationary weapon, `BlockLeftStance` (none of the install's weapons).
- `ShiftWeaponRoot`'s branch for an animated interaction or the left hand in use (the weapon by the authority alone, its local turn the procedural motion's), and its position cache (`POSITION_CACHE_FOR_WEAPON_PROCEDURAL`): neither was built before the stance either.
- The aim plane's matrix in the camera's parent's axes with the curve up (`CameraTransform.parent`): taken in the weapon root's parent's, as before (the MDR's aimed camera within 0.3 mm of the game's).
- The packet and others' view of it.
