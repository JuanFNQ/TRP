# Footsteps and the other sounds of moving

What the game plays for the player's own feet and kit as the player moves, stops, turns, jumps, lands, crouches and crawls; how it knows what the feet are on; and how loud it plays each in first person. Asked for by the user on 2026-10-16 ("Lets add the sounds for moving in the world. May want to add different materials on the ground to walk on. Look how tarkov handles footsteps"). Read from the game's code (`local\decomp\src`: `Player`'s sound part, `MovementContext`, `PhysicalBase`, `SoundBank`, `SimpleSource`, `ReverbSimpleSource`, `BetterAudio`, `ClientAudioSettings`) and from the install. Nothing here has been set beside the running game ("Compared with", at the end).

## Where the sounds are kept

One object, `Sounds`, in `assets/content/audio/prefabs/movement/sounds.bundle` (the player loads it in `InitAudioController`). It holds:

- **A set of banks for each surface** (`Sets`, each a `SurfaceSet`): run, sprint, stop, landing, turn, duck, prone, prone drop, jump (and the vault's, below). The install has a set for each of the twenty surfaces. The player keeps them by surface and begins on concrete's.
- **The kit's banks**: `Gear`, `GearMedium`, `GearFast`, `BackpackDrop`.

**A bank** (`SoundBank`, a script object of its own) has a base volume, a volume taken off at random (`DeltaVolume`), a base pitch and a pitch spread (`DeltaPitch`), a rolloff, and its clips: for each environment three lists (`Environments[n].Clips`, each a `DistanceVarity`). The player passes the outdoor environment (the first) on every call, so only the first environment's lists are ever played. A bank may have blend points (`BlendOptions`, a `DistanceBlendOptions` object: four numbers) that choose among the three lists by a value ("Which clip", below). One bank object may serve twice: the swamp's run bank is its duck bank too.

What the install has, read with `probe stepsounds`: on concrete the run bank holds no clip in its first list, six walking clips in the second and six running clips in the third, with blend points −0.2, 0.4, 0.6, 1.01; the duck bank holds six in its second list, with −0.01, 0.2, 1.01, 1.01; every other bank holds its clips in the first list and has no blend points. Base volumes: run, sprint, stop and duck 0.68 (0.6 on snow), landing, jump and prone drop 0.8, turn 0.45, prone 0.7 to 1; every bank's pitch spread is ±0.15. All 1,272 clips are 16-bit PCM or IMA ADPCM (`sound.md`, "Where the sounds are kept"); none is Vorbis.

**The surfaces** (`BaseBallistic.ESurfaceSound`, by number): Concrete 0, Metal 1, Wood 2, Soil 3, Grass 4, Asphalt 5, Glass 6, Gravel 7, MetalThin 8, Puddle 9, Slate 10, Tile 11, WoodThick 12, Swamp 13, Garbage 14, GarbageMetal 15, Plastic 16, WholeGlass 17, WoodThin 18, Snow 19.

## The surface under the feet

The surface is looked for each time a sound of the feet is about to play (`CheckSurface`; for a first-person player it always goes on, as one is never out of its own range). A line is cast from the middle of the character's collider straight down, the collider's height and half a metre long (`CalculateMovementSurface`), against the layers the steps listen to (`AudioControllerStepLayerMask`). What it hits first decides:

- **nothing hit**: the surface stays what it was;
- **something with a ballistic component** (`BaseBallistic`): that component's surface for the point hit (`GetSurfaceSound`; a terrain's may vary over it);
- **something without one**: concrete.

When the surface changes the player takes that surface's set. It also marks the surface soft or not (`MovementContext.SoftSurface`: asphalt, concrete, gravel, soil, wood, thick wood and puddle count as soft, which reads backwards but is the code), which only the Covert Movement skill's figures go by.

## When each sound plays

The movement states run each frame (`StateChangedHandler` starts and stops a loop of its own for idle, run and sprint). In this project's names: the game's `Run` is `Move`, its `Transition` out of a sprint is `SprintStop`, `Transit2Prone` is `ToProne`, `Prone2Stand` is `FromProne`, `ClimbUp` is `Climb`, `VaultingLanding` is `VaultLanding`.

| When | What plays | Where in the game |
|---|---|---|
| Moving (`Run`), each footstep | the surface's run bank, or its duck bank when crouched low; 0.1 s later the `Gear` bank | `Run`, `PlayStepSound`, `Gear` |
| Leaving `Run` with no footstep heard in it, more than 0.66 s after the last | a footstep as above | `StateChangedHandler` |
| Sprinting, each footstep | the sprint bank and, at once, the `Gear` bank | `Sprint` |
| Leaving a sprint with no footstep heard in it | the sprint bank | `StateChangedHandler` |
| Leaving a sprint for its stop or for idle | the stop bank | `StateChangedHandler` |
| Entering a jump | the jump bank and the `GearFast` bank | `StateChangedHandler` |
| Coming down onto the ground | the landing bank, if the drop was more than 0.3 m | `OnGrounded`, `PlayGroundedSound` |
| Going prone out of a sprint | the prone drop bank | `StateChangedHandler` |
| Going prone otherwise; getting up from prone | the `GearFast` bank | `StateChangedHandler` |
| Crawling, at the crawl clips' marks | the prone bank | `PlaySoundBank("Prone")` |
| Standing still, the view turned more than 45 degrees from the body | the turn bank, and `Gear` with it if loud enough | `Idle`, `PlayTurnSound` |

**A footstep** is the body animator's `FootStep` curve changing sign (each clip of walking and running carries it), heard if the last footstep was more than 0.2 s before and the character has been falling for less than 1 s (0.6 s in a sprint). This is the same footstep the weapon's bob is timed by (`body-animator.md`; `Run` and `Sprint` set `ProceduralWeaponAnimation.Walk.StepFrequency` in the same place). The time of the last footstep is set by a footstep heard, by the footstep played on leaving `Run`, and by a crawl's sound; the bob's next interval is measured from it.

**Crouched low** is a smoothed stance under 0.11 (`Player.Pose`: `Duck`); above that a crouched walk plays the run bank.

**The crawl's marks** are animation events in the crawling clips (`Prone_0` and the rest of the `Prone` state's blend tree): a call `PlaySoundBank` with the word `Prone`, at 0.40 s and 1.18 s of `Prone_0`'s 1.6 s. A mark plays the prone bank if the last footstep or crawl sound was at least 0.5 s before; it then counts as the last.

**The turn** is checked on every frame of `Idle`: if the hands are more than `TURN_ANGLE` (45 degrees, the game's hard settings) from the body, the turn sound is tried, and the check waits `TURN_SOUND_DELAY` (0.5 s) before it looks again. It plays if the last turn sound was at least 0.6 s before (`maxLengthTurnSound`).

**The landing.** The movement's ground check, on the first frame on the ground after being off it, gives two heights: how far below the highest point since leaving the ground the feet are now, and how far above the place they left the ground (`MovementContext.CheckFlying`). The player takes the larger (only the first after a climb up), and plays if it is more than 0.3 m (`LandingThreshold`). Nothing plays within 0.5 s of the last landing sound, nor while the state is a climb up, has just been one, or has just been a vault's landing (`IsPrevStateClimb`), unless the state is `Run` or `Sprint`.

## How loud

Every figure below is a share of the clip's own level (1 is the clip as it is) before the source and the mixer ("From the feet to the ear"). In first person:

**The movement's multiplier** for a kind of movement (`CalculateMovementVolumeDefaultMultiplier`): the sound radius, times the kind's multiplier, times a speed share for footsteps, times (1 − the Bot Sound skill's buff, 0 untrained). The game writes it as a rolloff over 70 m divided by 70 (`DEFAULT_FP_ROLLOFF`), which cancels in first person.

- **The sound radius** (`Physical.SoundRadius`): `Stamina.SoundRadius` of `globals.json` (1 to 1.3) at the overweight (`BaseOverweightLimits`).
- **The kind's multiplier** (`PlayerSettings.MovementRolloffMultipliers` in SPT's `settings.json`): run 0.85, sprint 0.95, stop 0.85, land 1, turn 0.8, duck 0.4, prone 0.5, drop 0.95, jump 0.5, gear 0.5.
- **The speed share** (footsteps only): from 0.8 at a standstill to 1 at the top walking speed, by the speed setting over the top speed (`MinSpeedRolloffMult` and `CalculateSpeedMult`).
- **Which multiplier a footstep takes.** The player keeps each kind's last multiplier and a footstep's volume takes the one kept before it is worked out again for this footstep with this footstep's speed: a footstep is as loud as the speed of the footstep before it says. The first footstep of a run takes the run's without the speed share (the source was set up for running when made). Every other sound works its own out first.

**SPT's two figures that the game does not read.** `settings.json` also gives `MinStepSoundVolumeMult` 0.2 and `MinStepSoundRolloffMult` 0.5, but the game's class names its fields `MinSpeedVolumeMult` and `MinSpeedRolloffMult`, so they keep the class's own 0.15 and 0.8. (Read from the code; the names differ, nothing else maps them.)

The volumes, each then clamped to 0 to 1 by the bank (`SoundBank.Play`):

- **A footstep**: the Covert Movement factor (1 untrained), times the speed share of volume (the speed setting over the top speed, at least 0.15, at most 1), times the movement's multiplier (run or duck), times the bank's base volume less up to its `DeltaVolume` at random.
- **A sprint's footstep**: the movement's multiplier (sprint), times the bank's base volume, times 0.5 + 3 × the overweight.
- **The stop**: the bank's base volume times the movement's multiplier (stop).
- **The jump**: the movement's multiplier (jump).
- **The landing**: from 0 at a drop of 0.1 m to 1 at 0.75 m (2.5 times the threshold), times the movement's multiplier (land).
- **The prone drop**: 0.7 times the movement's multiplier (drop).
- **A crawl's mark**: the larger of 0.4 and the speed setting times the prone bank's base volume, times the movement's multiplier (prone).
- **The turn**: the average turning speed over the last ten frames, in degrees a second, taken as a share of 1 to 360 (to 720 crouched fully: 360 + (1 − stance) × 360), times the turn bank's base volume, times the movement's multiplier (turn). The average is of the yaw's change each frame over the frame's time, signed and not clamped, so **a turn to the left averages below zero and is silent** (`GetAverageRotationSpeed` takes the larger of it and 0). That is the code, kept.
- **The kit** (`PlayGearSound`), its own source: the volume asked, times the movement's multiplier (gear), times the equipment's noise (1 + the overweight, less the Covert Movement buff). It is asked for at 1 with a sprint's footstep; at 0.1 to 0.5 with a footstep (the speed setting × 0.75 × √stance, standing; the speed setting up to 0.3 crouched low), 0.1 s after it; at the equipment's noise with a jump (so the noise counts twice), fast; at 0.7 going prone or getting up, fast; at the turn's volume with a turn whose share is over 0.4; at 0.2 when the aim's call brings the sights up or takes them down (`FirearmController.SetAim`; since 0.41.0, `sound.md`, "The aiming's sounds"). Not while dead.

**The speed setting** is the selected speed after its limits (`ClampedSpeed`), and a footstep's own blend value (below) is the larger of it and the slowest step (`Physical.MinStepSound`: 0.6 × the overweight, or 0.6 with a leg damaged).

## Which clip, at what pitch (`SoundBank`)

**The list.** A footstep passes its blend value; every other sound passes 0 (the distance to the listener, which is 0 in first person). With no blend points the first list plays. With them, the value is placed among the four points: under the first, the first list; between the first and second, a clip of the first list and one of the second at once; between the second and third, the second list; between the third and fourth, the second and third lists at once; past the fourth, the third. Two lists at once share the volume by where the value lies between the two points (the first list's share falling from 1 at the lower point to 0 at the upper). A list that is empty gives no clip, and **a sound with one clip plays it whole**: the source plays a pair's clips at their shares only when both are there (`SimpleSource.Play`). So on concrete a footstep under a blend value of 0.6 plays a walking clip whole, and from 0.6 to 1.01 a walking and a running clip together, the running one growing.

**The clip in a list.** Each bank keeps one shuffled order of as many places as the list it last shuffled for, and takes them from the end; when they run out it shuffles again for the list now asked. The shuffle swaps each place with one drawn from all but the last place (`Random.Range(0, n − 1)`, whose top is left out), so the last place never moves first and two clips simply alternate. A place past the list's end wraps round. A list of one clip plays it.

**The pitch**: the base pitch plus up to the spread either way, drawn for each sound, set on the source.

## From the feet to the ear

The footsteps and the kit play from two sources of the `Character` group (`BetterAudio`: a `ReverbSimpleSource` from `Audio/AudioSourceSpatialBuiltinEQReverbPrefab`), each 0.1 m over the feet and going with them, sent to the mixer's `ClientPlayer/ClientPlayerMovement` group. The kit's source plays at 0.85 (`FP_GEAR_VOLUME`); the steps' at 1 (the group's preset `OverallVolume`, 1 in SPT's `settings.json`).

- **In first person the sources are not binaural** (`EnableSpatialization` off, HRTF 0), but the group's preset has a spatial blend of 1, so the engine's own 3D sound applies: the prefab's source falls off by distance logarithmically from 1 m (its `rolloffMode` 0, `MinDistance` 1; read from `resources.assets`), with no Doppler. The listener is the camera (`AudioListenerConsistencyManager` follows `CameraManager`'s camera). Standing, the eye is about 1.5 m over the source and a step comes at about two thirds of its level; crouched low or prone, at its whole.
- **Panning**: the engine's own (FMOD's) law for a source to one side of the listener is not read (`PROVISIONAL`: equal power by how far to the side the source lies, so a source in the middle gives each channel 1/√2, as measured for the interface's mono sounds in `sound.md`).
- **The pitch** is the source's, so a sound still playing when the next sets a new one changes with it (`PROVISIONAL`: the engine's handling of a one-shot sound after its source's pitch changes is not read).
- **The reverb**: in first person the same clips go to a second source of the prefab's (`ReverbSimpleSource._reverbSource`) that Meta's acoustics plug-in turns into the room's reverberation only. Not built.
- **The mixer**: the `ClientPlayerMovement` group is at -3.5 dB in the start snapshot, with every group above it at 0 dB (read 2026-10-16, 0.40.0: `sound.md`, "The game's mixer"; until then taken as adding nothing); its two equalisers are not built; the master group takes the overall volume (`sound.md`, "How loud") and compresses over -1 dB.

The bank's rolloff and the movement's rolloffs set how far others hear the player and do not change what the player hears of itself.

## What the app has

**The range's ground** is concrete, as the game's default, and so is every box of it but the tables, the shelf and the wooden crate (wood), the car and the airdrop crate (metal), the gun cases (plastic) and the doors' leaves (wood). **A sampler of all twenty surfaces** lies on the ground to the right of the weapon tables' arm (`--start surfaces` stands before it): twenty squares of 1.8 m in four columns and five rows, by the surfaces' numbers from the far row, each coloured and labelled with its surface's name. A square is only drawn; it is a surface where the line down meets the ground inside it. The sampler, its squares' places, sizes and colours, and what the range's boxes are made of are the app's.

The simulation (`trp_sim::steps`) runs the player's rules on every step of the movement (the replay's too, as two of them set the time of the last footstep the bob goes by), which asks the world what the line down meets (`World::surface`), and hands the host what to play: a bank by surface and kind, a volume and a blend value. The bank's choosing (`trp_sim::steps::choose`) gives the clips, their shares and the pitch; the app plays them (`step_sound.rs`, into the one source of `ui_sound.rs`, whose sounds are kept as read, 16 bits, and read at their own rate and their source's pitch by straight lines between frames: `PROVISIONAL`, the engine's resampling not read). The crawl's marks are the body animator's: every clip with a share above none passes its own events as its time goes over them (`PROVISIONAL`: the engine's rule for the clips of a blend tree and of a transition is not read; the 0.5 s between marks makes two clips of the blend one sound). `--sound-out` keeps what a run mixed and `--log-sounds` says what it played (`manual.md`). The options' `MOVEMENT SOUNDS` switches all of it off (`settings.md`, "Features").

## Not built

The vault's and climb's sounds (`SurfaceSet.VaultSoundSet` and the rest, `BaseVaultingAudioController`); a season's layer over the steps (`PlayerStepLayerAudioController`, snow over a surface in winter); the room's reverberation; the mixer's own groups; being heard by others (the rolloffs); the Covert Movement and Bot Sound skills (untrained: their buffs are 0); `GearMedium` (mounting) and `BackpackDrop`; muffling by headgear (`UpdateMuffledState`); a terrain whose surface varies over it.

## Compared with

**Nothing of the game's.** The rules, figures and clips are read from the code and the install; no recording of the game's sound in a raid has been made. A short recording in a raid of walking, running, sprinting, crouching and jumping on known surfaces would settle the levels (the mixer's group, the falloff and the pan) as the user's recording of 2026-10-16 settled the inventory's.

**The app against its own rules** (0.36.0): an install test (`crates/trp-sim/tests/steps.rs`) walks, sprints, stops, jumps and crawls with the real body animator: footsteps 0.37 to 0.40 s apart walking and 0.28 to 0.30 sprinting, the kit 0.1 s after each walking one and at once with each sprinting one; one stop on letting go; the jump with its kit, and a landing at 0.40 for a drop of 0.36 m from the jump's top; the crawl's marks 0.78 to 0.83 s apart (two a cycle of `Prone_0`'s 1.6 s). A run across the sampler heard each square it crossed in turn (concrete, whole glass, swamp, puddle, asphalt, metal, concrete), and its own tape had each footstep within 9 ms of where its log put it at 0.77 to 1.0 of the gain asked; the kit's sounds at 0.76 to 1.0, matched only in part (`docs/workbench.md`).
