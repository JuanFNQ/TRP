# Workbench

How to work in the workspace: commands, the kept recordings, the scratch scripts under `local\`, and the traps. Moved out of `AGENTS.md` on 2026-10-10.

Run commands from `E:\stlkreft\TRP`. The install path comes from `EFT_DIR` or `local\eft_dir.txt`.

```
cargo build --workspace
cargo test -p trp-data
cargo run -p trp-app -- --view side --play akm_check_0 --at 2.0 --screenshot local\shots\x.png
cargo run -p trp-data --bin probe -- weapon 59d6088586f774275f37482f
```

- `--screenshot` saves one frame and exits. Read the PNG to check a rendering change; do not judge it unseen.
- In PowerShell 5.1, do not use `2>&1` on `cargo`. Redirect with `*>` to a file under `local\` and read that.
- The Bash tool cuts a long here-document short (2026-10-10: a `cat > file <<'EOF'` of some 150 lines ended in "here-document delimited by end-of-file" and nothing after it ran). For an edit script or a block of test code longer than a screen, write the file with the Write tool into the session's scratchpad and run or append that. And on this Windows `perl -pi` and `sed -i` did nothing to a file under `%TEMP%` without saying so: make the change with the Edit tool or rewrite the file.
- `cargo build` cannot replace `target\debug\trp.exe` while the app is open ("Access is denied"), and the rest of the build goes through, so a capture taken next shows the old app. If the user has the app running, wait; never close it for them.
- A capture's frame numbers hold only between runs made the same way: with `--log-pacing` the run asks for the focus and draws at the screen's rate (165 a second here), without it the same script passed frame 635 a second later. To capture a moment, log one run, read the frame of the moment off the log (`awk` on the `state` column), and take the capture with `--log-pacing` on as well.
- A file put back from a copy keeps the copy's old time (`Copy-Item`, `cp -p`), and cargo then sees nothing to rebuild: the next run tests the code as it was before the file was put back. Touch a restored file, or check the binary did rebuild (2026-10-10: two "it passes either way" results were the same stale binary).
- Bevy is pinned to 0.16 (0.16.1 builds on rustc 1.99). 0.19 is current. Upgrade deliberately, not in passing.
- The first Bevy build takes about 3 minutes; later builds take seconds.

Layout:

```
crates/trp-data   readers: install (where the install is, and the player's own view settings in it),
                  unity/ (bundle, serialized, typetree, value, assets, extract, texture, anim),
                  database (SPT JSON), weapon (assembly), states (a whole animator controller:
                  parameters, states, transitions, blend trees, layers, marks), body (the body's
                  controller and the rest pose of its bones), arms (a skin: the arms', a piece of
                  clothing), dress (what is worn over it: a skin or a model for the head), player (tuning on the player
                  prefab, the turn-away's poses, its two limb solvers and four twist bones, the
                  character's capsule, the camera's box), rootmotion (the table, and the blend-weight functions), hard_settings,
                  engine, curve; bin/probe
crates/trp-sim    the controller with no engine types: animator (the engine that runs a controller),
                  body (the body's controller on it), movement, hands (weapon motion), recoil, breath,
                  firearm (trigger, rounds, reload), stamina (the three pools and what drains them),
                  vaulting (the grid of rays, the obstacle it measures and the choice of vault or
                  climb; the moves themselves are movement states), arms (the game's limb solver and
                  twist bones),
                  rig (the weapon's rig and its clips with no engine, for the replay), script (the
                  recorder's scenario files, for the replay and for a host to play), settings,
                  math (Unity's functions); bin/replay
crates/trp-app    Bevy app: main, controller (input and placement), hands_view (weapon motion onto the
                  rig and camera), arms (reads the rig for the simulation's arm solver and writes its
                  answer), weapon (spawning), body_view (the body's clothing skinned to a skeleton the
                  body's animator poses), range (the test range's solid boxes and floors, as the world
                  the simulation asks its questions of), hud (the stance, speed and stamina display),
                  render_data, animation, convert
hosts/spt-oracle  C# BepInEx plugin that records the game; scenarios/*.json are shared with bin/replay
docs/spec         movement.md, body-animator.md, weapon-handling.md, firing.md, hands-on-weapon.md, stamina.md,
                  vaulting.md
```

The recordings are kept in `local\traces\recording3` (19 traces), `local\traces\recording4` (28 traces, recorder 0.2.0), `local\traces\recording5` (28 traces, recorder 0.2.1), `local\traces\recording6` (40 traces, recorder 0.3.0, 185 MB), `local\traces\recording7` (49 traces, recorder 0.4.0, 262 MB), `local\traces\recording8` (56 traces, recorder 0.4.1, 309 MB), `local\traces\recording9` (2 traces, recorder 0.4.2: scenarios 53 and 54 alone), `local\traces\recording10` (56 traces, recorder 0.4.4, 402 MB) `local\traces\recording11` (59 traces, recorder 0.4.6, 454 MB) and `local\traces\recording12` (59 traces, recorder 0.4.7, 460 MB), all ignored, because each new recording overwrites the files in `E:\spt\BepInEx\plugins\traces`. Copy a new recording there first. `replay --traces local\traces\recording8` and the scripts' `-Traces` option read them; run all ten after a change to the simulation, since each has things the others lack (the third ran into obstacles and over slopes; the fourth has prone, the turns and the reloads with the view 6 degrees down; the fifth was made facing the other way on the fourth's ground, which is what rolls its lying body; the sixth has the view level, the aiming cases, the sidestep and the body's bones; the seventh has a character that had not yet sprinted, the turns under each state's limits, the reloads and looks in a sprint and lying, and the elbows' goals; the eighth has the aimed rolls where the camera's box rests on the ground, the trigger in a sprint and a crawl, the turn in a reload lying down and the jump out of a sprint; the ninth has the weapon pushed back by the ground, lying and crouched; the tenth has the legs' bones, the ground under the feet on every frame, the game's own weapon-line length, the view 9.5 degrees down from the start, and the sights let go of while pushed back; the eleventh has the turns on the spot under a mouse's kind of input (small turns back, flicks at 420 degrees a second, a reversal in mid-turn, the same looking 60 degrees down and crouched), which is also where the sway's timing shows, and the character's hunger and thirst moving the stamina's rates over eleven minutes). The twelfth has the character's food and water, the oxygen and the skills on every frame, so stamina is compared there with nothing fitted. All ten take about seven minutes once built (run `target\debug\replay.exe` on each in turn in one background command, after `cargo build -p trp-sim --bin replay`). The fourth's to the eighth's folders each hold a `ground.txt` giving the ground's slope (`0 -0.00377`: the rise per metre along the world's x and z); `replay` reads it and without one takes the ground as level. The eighth's has a third number, 0.0762: how far over the ground the character stood on the spot the scenarios start from (the depth its standing jump, scenario 08, lands at). The replay's ground is what the character rides on; with the third number it puts the terrain that much lower for what meets the terrain (the camera's box, the weapon's line), in scenarios that have no jump of their own to find the ground by and start with a ground check that reaches it. A recording on other ground needs its own: from 0.2.2 on the recorder writes `surface_normal`, and the rise is `-normal.x / normal.y` and `-normal.z / normal.y`. A recording's folder may hold two more things. `weapon_length.txt`: the length of the game's weapon line in that recording, one number, for traces older than recorder 0.4.3 (which writes it into the header itself); the ninth's is fitted and says so. And a `scenarios` folder with the scripts the recording was made with, where those have been rewritten since: `replay` takes a scenario from there before the shared folder (the eighth keeps its own 53 and 54 so; without them the replay played the new scripts against the old traces and 53 came out 14% in the wrong state). **When a scenario's script is rewritten, put the old one in the folder of every recording made with it first.**

Scratch scripts in `local\` (ignored, PowerShell; all take `-Traces <folder>`): `table.ps1` replays every scenario against the traces and tabulates the gaps (distance, the game's velocity figure, movement state, eye, weapon, camera, the body's angle to the view); `mm.ps1` does the same in millimetres with the share of frames on which the movement state and the two animators' states differ; `cmp.ps1 -Scenario 06 -From 1.3 -To 2 -Numbers speed,smoothed_speed` sets recorded and simulated values side by side over time; `frames.ps1 -Scenario 01 -From 420 -Count 40` does it frame by frame with what actually moved; `wpn.ps1 -Scenario 26 -Frames 320,400` prints the weapon bone's place and turning on both sides in the same units; `rotdiff.ps1` works out what turn takes one to the other, in the rig's axes and in the bone's own; `aim.ps1 -Scenario 33 -Game aiming,firearm_aiming,sprinting -Sim aiming` prints a line whenever a flag changes on either side (it reads true and false, which `cmp.ps1` does not); `layers.ps1 -Scenario 13 -From 0.3 -To 1.2` prints the game's own animator layers from a 0.3.0 trace whenever one changes state; `ops.ps1 -Scenario 46 -Game prone,operation,p_ProneReload -Sim prone,p_ProneReload` is `aim.ps1` for values that are words (the recorder's `operation`); `arms.ps1 -Traces <folder>` replays and tabulates the solved arms, the elbows' goals and the hand markers against a recording that has them (0.3.0 on); `overlap_scan.ps1 -Traces <folder>` lists the scenarios of a recording in which the game's weapon was pushed back at all (0.2.1 on; it reads only the traces); `line.ps1 -Scenario 28 -From 9.55 -To 9.72 -Traces <folder>` sets the game's overlap depth beside how far over the ground the simulation's weapon line is at both ends (replay that recording first); `r6_health.ps1` checks a recording is whole; `trace_speed.ps1 -Scenario 09 -State JumpLanding -Traces <folder>` prints how fast the game's feet and eye move over the ground frame by frame around a change of movement state (the game's own evenness); `pace.ps1 -File local\pace_walk.txt -From 600 [-State Sprint] [-ShowLong]` sums up a `--log-pacing` file from the app (frame times, the simulation's cost, the camera's travel and the view's turn per drawn frame, and how uneven each is from one frame to the next) and `pace_rows.ps1 -File <log> -From <frame> -To <frame>` prints one frame by frame. `vec.ps1 -Scenario 53 -From 6.4 -To 9 -Every 12 -Names hands_pos,ribcage_p -GameOnly aiming,overlap_depth -SimOnly camera_push -Traces <folder>` sets any vectors or numbers of the two side by side to four decimals with their difference, flags too; `mmdiff.ps1 -Before local\r8_mm_after2.txt -After local\r8_mm_after4.txt` sets two saved `mm.ps1` tables side by side (the scenarios whose states differ in the new one, and how distance, eye and weapon moved on average), which is the regression check after a change: save a table per recording with `mm.ps1 -Traces local\traces\recordingN *> local\rN_mm_<name>.txt` (the tables as of the end of 2026-10-10 are `local\r3_mm_after17.txt` to `r12_mm_after17.txt`, with the replays' whole output beside them as `local\r3_replay_after17.txt` and so on; `after13` is the state before the twelfth recording was worked, `after14` with the stamina correction it led to, `after15` with vaulting in (no line of any table differs between those three), `after16` with the breath fed the stamina's levels (the weapon's two columns differ in the last place in a handful of scenarios; nothing else) and `after17` with the weapon's length measured at a reload's end (no line differs from `after16`; what it changed is `weapon_length_now`, which the table does not show: `local\gaps.sh <saved replay output> weapon_length_now` does); `after11` is the state at the end of 2026-10-09 and `after12` the next day's before stamina went in; `mm.ps1 -Parse <saved output>` tabulates one without replaying; rows 18, 19 and 24 always show a share of `hands %`, which is the weapon's animator, known; in the tenth, 02 is 0.83 m out, which is the recording's ground). They are the quickest way to find where the two part. `local\legs_game.sh <trace.jsonl> <from s> <to s>` (Bash) prints the game's feet in the world frame by frame from a trace that has the legs (0.4.4 on): time, frame length, left foot, right foot, the player's place. More Bash scripts in `local\`, from 2026-10-09 (run them with the Bash tool; each says its columns at its top): `feet_vs_game.sh <pace log> <legs_game.sh output> [shift s]` sets the feet the app drew in a `--scenario --log-pacing` run beside the game's, by the script's time (the app's turns start a frame or two apart from the game's, so try shifts of a few milliseconds each way, or cut the game's file to one turn first); `pace_grid.sh <pace log> [step] [from] [to]` puts such a log on an even grid of the script's time, and `feet_on_screen.sh <grid> <from> <to> [every]` works out where the drawn feet fall on a 1080-line picture; `shots.sh <trace>` lists every shot of a trace, the game's or the simulation's, with what the recoil's three springs did after it, and `shots_sum.sh <file> <who>` sums a file of those up; `kicks.sh <trace>` works each shot's force back from the springs' steps and `kicks_sum.sh` sums those; `shot_delay.sh <trace>` says how many frames after the rounds dropped the springs first moved, for the first shot of each run of fire. From 2026-10-10: `gaps.sh <saved replay output> <value> [value ...]` prints the largest and rms gap of the named values for every scenario of a saved report, one line a scenario (the quick way to see what a change did to one figure across a recording); `turn.sh <game trace> <sim trace> [from] [to]` lists every change of the body animator's state on either side with the view, the body and the angle between them, and flags a frame on which the two differ; `sway.sh <game trace> <sim trace> <from> <to> [every]` sets the sway spring side by side; `firstgap.sh <game trace> <sim trace> <vector> [tolerance]` prints the first frames on which a vector of the two differs, with the states there; `stam.sh <game trace> [every] [from] [to]` prints the game's two stamina figures over time with what bears on them and their change a second; `stamgap.sh <game trace> <sim trace> [tolerance] [from] [to]` prints a line wherever the gap in either pool changes. The simulation's traces write a lone number plain and a vector in brackets: a script that reads one has to know which. `replay --seed <n>` seeds the recoil's and breath's random numbers and `--out <folder>` keeps a run's traces apart (`local\oracle\seeds`, `local\oracle\fire4` to `fire10` hold such runs). A spring that looks at rest in a trace may still be moving in figures of 1e-40: test for rest and for movement with a tolerance, not for equality (two wrong tallies on 2026-10-09). `replay --layers <frame>` prints the body animator's layers on that frame. `replay --own-view` turns the view from what the recorder's script asked (0.4.0 on), through the simulation's own limits, instead of following the recorded view; the view is then compared as `look`. `replay --line <metres>` sets the length of the weapon's line for a recording that does not say (see `weapon_length.txt` above). In a replay's report, `<arm bone>_given_q` and `_given_p` are the solver fed the game's own palm and elbow goal (the solver by itself), and `<arm bone>_q` and `_p` the whole chain from the simulation's own weapon. In PowerShell do not name a function `RV` (it is an alias), quote scenario numbers (`-Only '01','06'`), and write `[math]::Min(1.0, $x)`, not `[math]::Min(1, $x)`: with a whole-number first argument PowerShell rounds the second to a whole number too (0.57 became 1, 0.4961 became 0, twice on 2026-10-09).

**The simulation's traces are the last run's.** `replay` writes them to `local\oracle\sim` whatever recording it was given, and a scenario the recording lacks is replayed at 60 frames a second with no recorded frames. So `cmp.ps1`, `aim.ps1` and `frames.ps1` set a recording beside whatever was replayed last: replay the recording being looked at first. (Running the third recording, which has 19 scenarios, leaves 20 to 40 written at 60 frames a second; read beside the sixth they look half a second early everywhere.)

Planned, not created: `eft-host` (C ABI and shared-memory protocol), `hosts/xray`.

`cargo test -p trp-sim -- --nocapture` prints what was measured: speeds, sway, recoil, rate of fire, reload time, what a wall does to the weapon. `--hold w+shift --screenshot-frame 300` runs the app with keys held and captures a frame; `--hold fire+auto+aim` fires on automatic while aiming; `--yaw -90 --hold w+aim --screenshot-frame 1500` walks into the range's wall with the sights up (a `--view side` capture there shows only the wall: the camera keeps beside the player and ends up behind it). A capture counts rendered frames, not simulation steps, and the frame rate varies between views, so pick a late frame and check the HUD in the image. **Run `cargo build -p trp-app` before a capture**: `cargo test -p trp-app` builds the tests and leaves `target\debug\trp.exe` as it was (2026-10-09: two captures of the camera's box showed the old build). **Pick a late frame:** for the first second or two of a run the engine is still compiling its pipelines and draws no weapon, sometimes nothing but sky (captures at frames 185 and 222 on 2026-10-09); `--start stairs --hold w+aim` is a capture that reaches the platform's edge 10 seconds in, at about frame 1667 at 165 frames a second. `--view body` captures the whole figure from three metres off (`--view side` is close in on the rifle); a walk straight ahead from the start is stopped by the ring's block 11 m out, 4 seconds in. A capture run (`--screenshot`) updates continuously whether or not its window has the focus, and with `--log-pacing` it asks for the focus; a played window without the focus is held to about 60 updates a second by the engine. To measure how evenly the app draws: `target\debug\trp.exe --hold w --turn 40 --screenshot local\shots\pace.png --screenshot-frame 3000 --log-pacing local\pace_walk.txt`, then `local\pace.ps1`; `--backend vulkan`, `--render-thread on` and `--present off` put back what was measured against. The same log can be written in a played run (no `--screenshot`), which is the only way to see what a real mouse gives. Since 2026-10-09 the log also has, per drawn frame and in the world's axes, the drawn body's facing, both feet, the pelvis and the point the view hangs at (`body_yaw`, `lfoot_*`, `rfoot_*`, `pelvis_*`, `rib_*`): what is drawn of the body can be measured, not only where the camera is. **A capture now starts at the install's own field of view (74) and head bobbing (1.0)**, not 50 and 0.2: pass `--fov 50 --head-bobbing 0.2` to compare with a capture made before.

More commands:

```
cargo run -p trp-data --bin probe -- weapon 59d6088586f774275f37482f marker     (the assembled weapon's bones whose path has the word, at rest)
cargo run -p trp-data --bin probe -- states 59d6088586f774275f37482f reload     (animator states, clips, marks)
cargo run -p trp-data --bin probe -- controller 59d6088586f774275f37482f 6 8    (the controller object, raw)
cargo run -p trp-data --bin probe -- bodystates sprint Player_anim_controller tree (body animator states; - shows all)
cargo run -p trp-data --bin probe -- bodyclips sprint_slow_0 tracks Spine       (a body clip's curves and bone tracks)
cargo run -p trp-data --bin probe -- bodystates - Player_anim_controller parameters (the body animator's parameters)
cargo run -p trp-data --bin probe -- blendcheck                                 (computed blend weights against the stored grids)
cargo run -p trp-data --bin probe -- hardsettings Preservation                  (the settings object, filtered)
cargo run -p trp-sim --bin replay -- --traces E:\spt\BepInEx\plugins\traces 06  (one scenario against its trace)
cargo run -p trp-app -- --hold reload --log-frames local\frames.txt --screenshot local\shots\x.png --screenshot-frame 700
cargo run -p trp-app -- --scenario hosts\spt-oracle\scenarios\59-turn-feet-down.json   (watch a recorder's script played in the app)
cargo run -p trp-app -- --scenario local\scenarios-app\vault-wall.json --yaw 90        (walks to the knee-high wall with the vault key held and over it, on to a block and up it)
cargo test -p trp-sim --test install vault -- --nocapture                              (what a vault, a climb and a sprint's vault measure)
cargo run -p trp-sim --bin replay -- --endurance 40 --strength 40 61-legs-dry          (a script with no trace, at the recorded character's skills)
target\debug\trp.exe --scenario <script> --yaw <bearing> --pitch <pitch> --log-pacing local\pace_x.txt  (the same, logged as drawn; ends with the script)
cargo run -p trp-sim --bin replay -- --traces <folder> --out local\oracle\seeds --seed 7 15-fire  (one seed of the recoil's random numbers)
cargo run -p trp-sim --bin replay                                               (replay all scenarios)
cargo run -p trp-sim --bin replay -- --traces <folder> --weight <kg>            (compare with game traces)
cargo run -p trp-sim --bin replay -- --traces <folder> --endurance 40          (the character's Endurance, where a recording never shows rested legs)
target\debug\trp.exe --hold w+shift --screenshot local\shots\x.png --screenshot-frame 5560   (a sprint held until the legs are spent, 30 s in)
& $dotnet build hosts\spt-oracle\Oracle.csproj -c Release -p:EftDir=E:\spt -v:q (plugin, into local\oracle)
```

The plugin's build output is `local\oracle\stlkreft-oracle`. Installing it means copying that folder to `E:\spt\BepInEx\plugins\`, which rule 2 leaves to the user.


## Added 2026-10-10, evening

The thirteenth recording is `local\traces\recording13` (60 traces, recorder 0.4.8, 548 MB with the vault traces); the four vault traces are in `local\traces\recording13-vault` (the wall 1.13 m high) and `local\traces\recording13-vault-low` (0.49 m), each with a `ground.txt` that says `profile`, and are replayed with `--scenarios hosts\spt-oracle\scenarios-vault`.

**The regression run** is now two scripts (Bash, from the workspace, after `cargo build -p trp-sim --bin replay`): `local/regress.sh <name>` replays every kept recording and the two vault folders and keeps each report as `local\r<N>_replay_<name>.txt` (`rv_` and `rvl_` for the vault folders), about eight minutes, best run in the background; `local/statecheck.sh <name> [earlier name]` then says, for each recording, in how many scenarios a state is off on any frame and which, the worst distance, eye and stamina gaps, and, given an earlier run's name, every scenario whose distance, eye, stamina or weapon gap moved by more than a tenth of a millimetre. The runs kept are `after17` (before this session) and `after18` (after it: identical for the third to the twelfth). Do not rebuild the replay while `regress.sh` runs: `cargo test -p trp-sim` relinks it too.

More Bash scripts under `local\`: `vault.sh <trace> [every]` lists every change of movement state, vault key, the animator's two vault flags, the ground flag and the sprint's flag in a game trace or a simulation's, with the obstacle as measured, the capsule and the stamina; `profile.sh <game trace> [from frame] [to frame] [every]` prints the ground along the path walked (how far out, the height, what lay under the character, the capsule, the speed); `side.sh <game trace> <sim trace> <from frame> <to frame> [every]` sets the two side by side a line a frame: state, animator state and time, how far out and how high, speed, grounded, the capsule, and the gaps along the path and in height. It is the first thing to reach for when a move differs.

`replay` writes more beside a recording since this session: `pos`, `grounded`, `capsule` (height and centre) and the vaulting parameters (`p_VaultingHeight`, `p_DoVault` and the rest).

The Bash tool mangles backslashes and brackets in a here-document's `awk` or `sed` program (the first listing script of this session printed nothing but dashes). Write such a script with the Write tool, or keep its patterns free of `\[`.

In the app: `--grid off` and `--fps off` put the range and the screen back as captures made before this session have them; `local\scenarios-app\vault-wall.json` played with `--yaw 90 --view body --log-pacing <file> --screenshot <png> --screenshot-frame 1330` goes over the low wall, up a block and off its far side (the states are in the log's sixteenth column).

## Added 2026-10-11

```
cargo test -p trp-sim --lib collision                                         (the line form and the box form of the game's collision)
cargo test -p trp-app range -- --nocapture                                    (the range: the stairs, the platform, the low wall)
cargo test -p trp-sim --test install the_legs_dry -- --nocapture              (61-legs-dry through the simulation, its gates decided by the simulation's legs)
cargo test -p trp-sim --test install a_starting_character -- --nocapture      (both Standard starting outfits from the install: slots, cells, weights)
cargo run -p trp-data --bin probe -- item <id or part of a name>              (an item of the database)
MSYS_NO_PATHCONV=1 target/debug/probe.exe json templates/profiles.json /Standard/usec/character/Inventory 1
cargo run -p trp-app -- --loadout Standard/usec                               (a starting outfit: carried, weighed, worn; Tab opens the inventory)
target\debug\trp.exe --loadout Standard/usec --inventory --screenshot local\shots\inv.png --screenshot-frame 400      (the screen, captured)
target\debug\trp.exe --loadout Standard/usec --view body --screenshot local\shots\body.png --screenshot-frame 400     (the outfit on the body)
target\debug\trp.exe --scenario local\scenarios-app\vault-wall.json --yaw 90 --log-pacing local\pace_vault.txt        (vault, climb and fall in the app, logged)
target\debug\trp.exe --start stairs --hold w --screenshot local\shots\x.png --screenshot-frame 1500 --log-pacing local\pace_stairs.txt   (up the stairs and off the platform)
```

In a pacing log the movement state is column 16 and the drawn feet are columns 18 to 20 (x, height, z in the engine's axes, z behind): `awk 'NR>1 { if ($16 != s) { print $1, $2, $16, $18, $19, $20; s = $16 } }' <log>` lists every change of state with where the feet were.

**Recorder 0.4.9** is built in `local\oracle\stlkreft-oracle` (all 60 scenarios) and, with `61-legs-dry` alone, in `local\oracle\legs-only` (about 45 s, no magazine; stand with 9 m clear behind and 26 m ahead). A recording made with it has `script_t` on every frame; `t` is then the seconds since the scenario began, gates and all. `local\traces\recording13\scenarios\61-legs-dry.json` is the old script, kept for that recording.

**The regression runs kept:** `after18` (the end of 2026-10-10), `s16` (after the collision work) and `after19` (the end of this session). `s16` is byte for byte `after18`. `after19` is too for the thirteenth and the two vault folders, and for the third to the twelfth it differs in three lines each: `61-legs-dry`, which those recordings have no trace of, is played by itself, and the script has changed (`bash local/statecheck.sh after19 after18` shows no scenario moved).

Captures of this session: `local\shots\s16_inventory.png` (the screen), `s16_loadout_body.png` (the outfit worn), `s16_stairs2.png` (the new stairs). Logs: `local\pace_s16_vault.txt`, `local\pace_s16_stairs.txt`.

## Added 2026-10-11, later

```
cargo test -p trp-sim --lib inventory                                         (the model: rooms, sizes, slots, moves, stacks, the raid's rules)
cargo test -p trp-sim --test install a_crate -- --nocapture                   (the app's crate emptied onto a bare character by the model's moves)
cargo test -p trp-sim --test install the_equipment_panel -- --nocapture       (the game's own equipment panel as read from the install)
cargo test -p trp-sim --test install a_recorded_equipment -- --nocapture      (a recorder 0.5.0 equipment list read back; made up, not recorded)
cargo run -p trp-data --bin probe -- scenes ui                                (the build's scenes whose path has "ui", and the equipment panel as read)
cargo run -p trp-data --bin probe -- screen level44 "Gear Panel" 1            (objects of a scene file by a part of their name, with layout and scripts)
target\debug\trp.exe --loot --bare --screenshot local\shots\x.png --screenshot-frame 300       (the screen with the crate open, nothing on)
target\debug\trp.exe --loot --screenshot local\shots\x.png --screenshot-frame 300              (the same with the crate's things on)
target\debug\trp.exe --loot --bare --drag 1100:180:170:450 --drag 1380:200:545:175 --drag 1120:360:545:545 --drag 632:175:790:680 --drag 170:450:248:175 --screenshot local\shots\x.png --screenshot-frame 280
                                                                               (five drags in a 1600 by 900 window: rifle to the sling, vest and backpack on, a magazine into the backpack, the rifle refused by the headwear's slot)
```

A drag's places are window pixels and move with the layout: after each drop the Gear page is drawn afresh (the backpack's slot is at 545, 430 before the vest is on and at 545, 545 after). Take them from a capture made just before. A scripted drag moves the real mouse pointer with it.

**The fourteenth recording** is `local\traces\recording14` (`61-legs-dry` alone, recorder 0.4.9, 2026-10-11 00:57; 5,459 frames; level ground, bearing 180.01; its `ground.txt` is `0 0`). `local/regress.sh` and `local/statecheck.sh` include it. `bash local/vault.sh local/traces/recording14/61-legs-dry.jsonl` lists its changes of state with the legs at each.

**Recorder 0.5.0** is built in `local\oracle\stlkreft-oracle` (all 60 scenarios, with `61-legs-dry` as rewritten) and, with `01-walk-forward` alone, in `local\oracle\inventory-only` (five seconds, no magazine; for the inventory's header). `local\oracle\legs-only`, `feet-only` and `vault-only` still hold older builds: copy the new `stlkreft-oracle.dll` into one before using it if the inventory is wanted from it.

**The regression runs kept:** `after20` (the height lag by the player's clock), `after21` (the inventory's moving and the changing weight: the same as `after20` in every line but the time taken to read the clips) and `after22` (the height lag's rule for lying down; the run to compare the next change with). `after19` is the last before the fourteenth recording.

**Reading a session that was cut off:** its transcript is `C:\Users\Owner\.claude\projects\e--stlkreft\<id>.jsonl`. Each line is an object with a `message` whose `content` is a list of parts: `text`, `tool_use` (with `input.command`, or `input.file_path`, `old_string` and `new_string` for an edit) and `tool_result`. Write the parts out with a few lines of PowerShell and read that.

Captures of this session: `local\shots\inv2_bare.png`, `inv2_equipped.png`, `inv2_dragged.png` (the screen: bare with the crate, with everything on, after five drags).

## Added 2026-10-11, afternoon

```
cargo test -p trp-sim --lib inventory                 (now also: a stack split, a thing put down and picked up)
target\debug\trp.exe --loot --bare --pitch 75 --drag 1100:180:170:450 --drag 1380:200:545:175 --drag 1120:360:545:545 --tap 136:ctrl:20 --drag 738:510:632:680 --tap 160:delete --tap 190:tab --tap 215:3 --tap 300:f --tap 330:tab --screenshot local\shots\x.png --screenshot-frame 430
        (everything out of the crate and on; a stack split with Ctrl; one half put down; the screen shut; the rifle drawn; the half picked up; the screen opened again)
```

`--tap <frame>:<key>[:<frames held>]` presses `tab`, `f`, `3`, `delete`, `ctrl` or `r` on a drawn frame of a capture run; drags begin at frame 90 and take 16 frames each, so a key meant for the third drag's release (frame 134) is held from a little before it. `--pitch 75` has the view on the ground 0.4 m ahead, where a thing put down lies.

`probe states 59d6088586f774275f37482f INVENTORY` shows the hands' states for the open inventory with their clips; `probe states 59d6088586f774275f37482f LActions` the left hand's gestures; `probe tree "assets/content/weapons/empty hands/weapon_empty_hands_container.bundle"` the empty hands' rig.

`local\README_before_shortening.md` is the README as it stood before it was shortened (its long parts are also at the end of `history.md`).

Captures of the afternoon: `local\shots\inv3_split_drop.png`, `inv3_in_hand.png`, `inv3_picked_up.png`.

## Added 2026-10-11, evening

```
target\debug\trp.exe --equip --tap 300:tab --tap 700:tab --log-frames local\jit_fixed.txt --screenshot local\shots\x.png --screenshot-frame 1000
        (the rifle in hand, the screen opened and shut: one line a drawn frame with the clip on screen, its time, and where the weapon and each hand are as the camera sees them)
target\debug\trp.exe --equip --hold w --tap 500:tab --log-pacing local\pace_walk.txt --screenshot local\shots\inv_walk.png --screenshot-frame 900
        (walking, the screen opened with the key still held: the state and the feet's place on every drawn frame)
cargo test -p trp-sim --test install a_screen -- --nocapture       (a walk and a sprint under an open screen, set beside keys let go)
cargo test -p trp-sim --lib firearm::tests                         (now also the hands' three states for the inventory)
cargo run -q -p trp-data --bin probe -- states 59d6088586f774275f37482f "Hands.IDLE"    (the resting state's ways out, the one into OPEN INVENTORY among them)
cargo run -q -p trp-data --bin probe -- clips 59d6088586f774275f37482f hands_o           (a clip's length and whether it loops)
```

- **Reading a frame log for a shake.** A column that climbs and snaps back at a fixed period is a clip being started again; the log's clip time shows the wrap. Columns of `--log-frames`, split on spaces: 1 the frame, 3 the time, 5 the clip, 6 its time, 8 the blend, 10 to 12 the weapon's moved bone, 14 to 16 the left palm, 18 to 20 the right. Of `--log-pacing` (which has a header line): 1 the frame, 2 the time, 16 the movement state, 18 to 20 the feet, 41 the body animator's state.
- **The bytecode of one method:** `dotnet local\tools\ilspycmd\pkg\tools\net10.0\any\ilspycmd.dll -il -t <Namespace.Type> -r <Managed folder> <dll> > local\decomp\il\x.il`. For `Assembly-CSharp.dll` it writes the whole assembly (166 MB) whatever the type: search it and delete it.
- **The kept regression is now `after23`** (the same reports as `after22`, byte for byte): `bash local/regress.sh <name>`, then `bash local/statecheck.sh <name> after23`.
- Captures of the evening: `local\shots\inv_walk.png` (the screen open over a walk that has stopped, the weapon held aside), `local\shots\jit_still.png` and `jit_fixed.png`; the logs `local\jit_still.txt` (the shake) and `local\jit_fixed.txt`, `local\pace_walk.txt`.

## Added 2026-10-11, late evening (empty hands)

```
cargo run -q -p trp-data --bin probe -- weapon "assets/content/weapons/empty hands/weapon_empty_hands_container.bundle"
cargo run -q -p trp-data --bin probe -- clips  "assets/content/weapons/empty hands/weapon_empty_hands_container.bundle" ""
cargo run -q -p trp-data --bin probe -- states "assets/content/weapons/empty hands/weapon_empty_hands_container.bundle"
        (the probe's weapon commands take a prefab's bundle, anything with a `/`, in place of an item's id)
cargo test -p trp-sim --test install empty_hands -- --nocapture
target\debug\trp.exe --bare --screenshot local\shots\eh_fp.png --screenshot-frame 400                 (empty hands, the view level)
target\debug\trp.exe --bare --pitch 70 --screenshot local\shots\eh_down.png --screenshot-frame 400    (the view down: a hand in the picture)
target\debug\trp.exe --bare --view body --screenshot local\shots\eh_body.png --screenshot-frame 400   (from outside)
target\debug\trp.exe --bare --hold w+shift --view side --screenshot local\shots\eh_sprint_side.png --screenshot-frame 420
target\debug\trp.exe --loot --bare --drag 1100:180:170:450 --tap 150:tab --tap 200:3 --tap 500:tab --tap 620:delete --tap 720:tab --log-frames local\eh_seq.txt --screenshot local\shots\eh_seq.png --screenshot-frame 900
        (empty hands; the rifle out of the crate onto the sling; drawn with 3; the inventory opened; the rifle put down with Delete, the pointer still on its slot; empty hands again)
```

- A capture of a held sprint has to be taken early: by drawn frame 700 the character has run the 12 m to the ring's block ahead and stands at it. Frame 420 is mid-sprint.
- The regression after the empty hands' work is `after24` (the weapon reader was split so that a prefab can be read without an item; the reports are compared with `after23`).

## Added 2026-10-11, the session after the evening's (the weapon's animator, the lean, the crate's kit)

```
cargo test -p trp-sim --test install the_weapons_animator -- --nocapture     (the AKM's controller on the engine: the draw, rest, the inventory down and back, with its marks)
cargo test -p trp-sim --test install empty_hands_are_fists -- --nocapture    (the kind of thing in hand: the body's share of the hands and a finger's joints, rifle against empty hands)
cargo test -p trp-sim --test install lean -- --nocapture                      (the lean keys as presses and releases)
cargo test -p trp-sim --test install a_crate -- --nocapture                   (the crate's kit made the rifle's own)
target\debug\replay.exe --traces local\traces\recording13 --out local\oracle\shadow 24-reload-empty
        (in the report: hands_anim, hands_anim_time, hands_blending and machine_entering are the weapon's animator run as a state machine, set beside the game's; hands_entering is still the logic's fixed sequence)
bash local/hstates.sh local/traces/recording13/24-reload-empty.jsonl local/oracle/shadow/24-reload-empty.jsonl 5.2 6.2
        (every frame on which the weapon animator's state, time or transition flag changes on either side, the game's then the simulation's; `<<<` marks a frame on which the states differ)
target\debug\probe.exe states 59d6088586f774275f37482f                        (the weapon's whole controller: layers, every state with its clips, marks with their conditions, and ways out)
target\debug\probe.exe bodystates Sprint Player_anim_controller tree          (the sprint's trees, the arms' layer among them: node 1.0 goes by WeaponTypeFloat)
target\debug\probe.exe bodyclips sprint_slow_0 tracks Palm                    (three clips of that name: their curves and the palms' and fingers' tracks)
MSYS_NO_PATHCONV=1 target/debug/probe.exe json templates/profiles.json /Standard/bear/character/Inventory/items 4 > local/bear_items.txt
bash local/bear_tree.sh local/bear_items.txt                                  (what a starting character's vest, backpack, pockets and container hold, as a tree of templates)
target\debug\trp.exe --bare --yaw 40 --pitch 40 --hold w+shift --screenshot local\shots\x.png --screenshot-frame 380
        (an empty-handed sprint looking down, clear of the crate: the left fist is in the picture; straight ahead from the start the crate stops the run 2.5 m on)
target\debug\trp.exe --loot --screenshot local\shots\x.png --screenshot-frame 300      (the crate's kit worn: AK55 magazines in the vest, PS x60 and x30 in the backpack)
```

- **Reading a mark's condition in `probe states`:** `if AmmoInChamber kind1 mode2 0/0/false` is kind (0 a whole number, 1 a float, 2 a flag), mode (0 equal, 1 not equal, 2 greater, 3 less, 4 at least, 5 at most) and the three values it might be compared with (float/whole/flag): here "the float `AmmoInChamber` is greater than 0".
- **The weapon animator in a simulation's trace:** `hands_anim` is the state's number as the recorder writes it (the low 24 bits of the hash of the state's own name; `IDLE` is 11622561, `DRY FIRE` 15771324, `RELOAD START` 16214692; `animator_state` in the replay works one out), `hands_anim_time` its time in cycles, `hands_blending` 1 in a transition. The simulation's trace has no header line; the game's has one (a script that pastes the two side by side has to skip one line of the game's only).
- **The regression runs kept:** `after25` (the kind of thing in hand: every report as `after24`) and `after26` (the weapon's animator beside the logic, the exit point as a crossing, the lean as presses and releases; compared with `after25` below). `bash local/regress.sh <name>`, then `bash local/statecheck.sh <name> after26`.
- Two replays must not share an `--out` folder. `local/exit_try.sh` is how an engine rule was tried both ways on the body's recordings behind an environment variable (the variable is gone again; the script is kept as a pattern).
- Scratch edit scripts of the session are `local\edit_*.pl` (exact-string swaps); the files they changed have a copy from before as `local\*_before_*.bak`.
- Captures of the session: `local\shots\eh2_fpd_380.png` (the left fist in a sprint), `eh2_sprint_fp.png` (the right hand), `eh2_sprint_side.png`, `kit_akm.png` (the crate's kit worn), `lean_keys.png`.

## Added 2026-10-11, night (the weapon goes by its animator)

- **The regression is kept as `after27`** (`local\r<N>_replay_after27.txt`). `after27a` is the same run made before the old clip player was taken out, and is identical value for value.
- `bash local/metricdiff.sh <name> <other name> [pattern]`: every compared value whose largest gap differs between two `regress.sh` runs, summed up by value: in how many scenarios it grew and shrank, the worst of each run, and where it moved most. `statecheck.sh` only looks at distance, eye, stamina and the weapon's place; this looks at everything.
- `bash local/metricgrew.sh <name> <other name> [pattern]`: for each value, the scenario where it grew most and by how much. Empty when nothing got worse.
- `local\akm_hands_states.txt`: every state of the AKM's hands layer with its marks and ways out (`probe states 59d6088586f774275f37482f "Hands."`). Game data: scratch only.
- `perl local/apply_edits.pl <file> <spec>`: replaces exact pieces of text in a file from a spec of `=====FROM` / `=====TO` / `=====END` blocks, with nothing to escape; stops before writing if a block is not found. The night's edits to the documents were made with it.
- **`--log-frames` now writes the animator's state**: the fifth field is the state the rig's animator is in or entering (spaces as underscores), the sixth how far through its state it is, the eighth whether it is between two states. `awk '{ if ($5 != last) { print $1, $3, $5, $6, $8; last = $5 } }' <log>` lists the states as they were drawn.
- **Scripted runs of the night:** `--equip --hold reload --tap 450:tab --log-frames ...` (a reload with the screen opened in it: ends in `IN INVENTORY` by `RELOAD INVENTORY END`); `--equip --tap 300:tab --tap 700:tab --log-frames ...` (down, held, back; the left hand's travel per drawn frame is fields 14 to 16); `--equip --hold fire+auto --log-frames ...` (`FIRE` and a frame of `IDLE` by turns); the whole flow with empty hands is in `inventory.md`. Captures: `local\shots\an_*.png`, `pics_fit.png`.
- `cargo test -p trp-sim --test install the_weapon_goes_by_its_animator -- --nocapture` prints the draw's, the put-away's and the reloads' timings on the install's own controller.

## Added 2026-10-12 (the warm-up, the gestures, recorder 0.6.0)

- **The regression is kept as `after28`**, identical to `after27` value for value.
- **The app logs how long its warm-up took:** `the renderer had built what it draws with 1.78 s into the run (250 frames)`. A capture's frame numbers count from the end of it.
- **`--log-frames` ends each line with `gesture <state> <how far> <share>`:** the state the body's layer of gestures is in or entering, how far through its state it is, and how much of the left hand is the body's. `awk '{ g = $(NF-2); if (g != last) { print $1, $3, g; last = g } }' <log>` lists the gestures as drawn.
- **A scripted walk to the crate:** `local\to_crate.json` (walk 0.82 s, then turn 90 degrees left and 51 down, asked at 1/0.7 of that since a script's turns go through the sensitivity) with `--scenario local\to_crate.json --scenario-at 0.5 --equip --tap 620:f`. The search key works in first person only (`--view body` sees no crate).
- `cargo test -p trp-sim --test install the_hands_work_a_container -- --nocapture` prints the gestures' timings on the install's own body animator.
- `local\body_states_all.txt`: every state of the body's animator with its ways out (`probe bodystates - Player_anim_controller`); the interactions' layer is machine 17. Game data: scratch only.
- **Recorder 0.6.0** (`local\oracle\stlkreft-oracle`, built 2026-10-12 02:50, not run). New in a frame: `p_IsInteraction`, `p_Interact`, `p_Approached`, `p_Inventory`, `p_InventoryOperation`, `p_Loot`, `p_Pickup`, `p_InteractionIndex`; `inventory_open`; `hands_controller` (the class's name); `working`, `working_state`, `working_animation`, `working_snap`, `working_grip` while a thing is being worked; `hands_left_progress`, `hands_use_left`, `hands_inventory`, `hands_active`; `hands_actions` (the left hand's layer of the hands' animator: state, time, weight); and `hands_anim` and its fellows for hands that hold no firearm. The body's `layers` array was there already: the interactions' layer is its twenty-first group of four (state, time, weight, next). In the header: `by_hand`.
- **A recording by hand** is `free-<date>-<time>.jsonl`; the replay does not take it (there is no script to replay). To read one: the frames where `p_IsInteraction` rises, where `working_state` changes and where `inventory_open` rises give the three times `interactions.md` asks for; `state` is the movement state's name on every frame.
- **`hosts\spt-oracle\scenarios-hands`:** six scripted scenarios (64 to 69); `cargo run -p trp-sim --bin replay -- --scenarios hosts\spt-oracle\scenarios-hands --traces <folder>` sets a recording of them beside the simulation, and without `--traces` plays them through the simulation alone.
- **A scripted pick-up:** `local\pickup.json` (crouch, then the view 68 degrees down) with `--equip --inventory --drag 740:520:740:520 --tap 130:delete --tap 160:tab --scenario local\pickup.json --scenario-at 2.0 --tap 720:f`: a stack is put on the ground out of the open screen (the drag onto itself only puts the mouse over it), the screen shut, and the stack taken up again (`8 Take` two frames after the key; `local\shots\gest_pickup_790.png`). Standing, a thing 0.4 m ahead on the ground is out of the game's reach of 1.3 m: crouch to it.

## Added 2026-10-12, later (the fifteenth recording, the recordings made by hand, recorder 0.6.1)

- **The fifteenth recording** is `local\traces\recording15` (recorder 0.6.0, 2026-10-12 03:39; the six scenarios of `hosts\spt-oracle\scenarios-hands`, 64 to 69; 6,356 frames; level ground, bearing -75.80; its `ground.txt` is `0 0`). Replayed with `--scenarios hosts\spt-oracle\scenarios-hands --traces local\traces\recording15`. `local/regress.sh`, `statecheck.sh`, `metricdiff.sh` and `metricgrew.sh` include it (`r15_replay_<name>.txt`).
- **The regression is kept as `after29`:** recordings 3 to 14 and the two vault folders identical to `after28` value for value; the fifteenth with nothing off in the weapon's animator, its logic or the hand markers.
- **The recordings made by hand** are `local\traces\byhand1` (nine `free-20261012-03....jsonl`, recorder 0.6.0, 03:26 to 03:33). By their times: `032651` a door (gesture 17), `032659` a door (20), `032842` a thing picked up, `032853` a container searched (19) with the rifle in hand, `032920` a reload from empty with the screen opened in it, `033211` a container searched (29) and the rifle taken off its slot, `033226` empty hands, the container searched and its lid shut (30), `033239` the rifle put back on its slot (a plain draw), `033306` the same with no round chambered.
- **`cargo run -p trp-sim --bin byhand -- <folder> [--from <s> --to <s>] [part of a name]`** reads each recording made by hand: what was carried beside the model's sum; what the body did, as recorded (gestures with how long their flag was up, movement states, the screen, the hands' and the body's flags for it, what the hands held and the weapon's operation); the states of the hands' animator as the game entered them beside the simulation's, with how much later each was; on how many frames the state differs and how far apart the time through it is where it agrees; and when the simulation's weapon appeared, went, and had its magazine out and in. With `--from` and `--to`, every frame between. The kept output is `local\byhand1.txt`.
- **How `byhand` drives the simulation,** each from what the recordings showed: the screen's opening and shutting is told before the frame's step; a reload, a put-away and a look when the game's operation changes, before the step; a new controller is made on the frame it shows and stepped in that frame unless the game's shows `SPAWN` at time zero (then it was made after the frame's update and is stepped from the next); the weapon's last step is taken on the frame the next controller shows; what has appeared is told of the screen after the step it appeared in.
- **`local/changes.sh <trace> <field> [field ...]`**: one line for every frame on which any named field changes (a word, a number rounded to two places, a short array); `name=` lists a field without having it start a line; `FROM` and `TO` in the environment bound the times. `bash local/changes.sh <trace> state operation hands_controller inventory_open p_IsInteraction p_InteractionIndex hands_anim` is the short of a recording made by hand.
- **`local/gesture_layer.sh <trace> [from s] [to s]`**: the body's layer of gestures through a recording (the twenty-first group of four in `layers`) beside the flag and the index that drive it, the left hand's share and the screen.
- **`local\akm_hands_numbers.txt`**: every state of the AKM's hands layer with the number the recorder writes for it (the low 24 bits of the CRC-32 of the state's own name; `perl -MCompress::Zlib -e 'print crc32("IDLE") & 0xFFFFFF'`). The body's layer of gestures: `Idle` 1448491, `17 Door Pull Hinge Left` 13953821, `29 Container Hinge Up Open` 4519655, `30 ... Close` 12006897, `8 Take` 8459582, `Inventory` 14469269.
- **Recorder 0.6.1** (`local\oracle\stlkreft-oracle`, built 2026-10-12 04:08, not run). New in a frame: `looking_at` (the kind of thing looked at); for the door or container last looked at, `thing` (its kind), `thing_id`, `thing_state` (the game's door state), `thing_angle` with `thing_angles` (shut, open), `thing_axis`, `thing_from_player` (metres, the world's axes), `thing_rot`, `thing_without_animation`, and what it asks of a player standing where this one stands: `thing_gesture`, `thing_snap`, `thing_grip`; and `keys_down`, the keys of a short list that went down in the frame. The `working...` fields of 0.6.0 are only there for a thing unlocked or kicked.
- **Scripted runs of the app:** `--equip --scenario local\to_crate.json --scenario-at 0.5 --tap 620:f --tap 1000:tab --log-frames ...` (the search: the crate's gesture two drawn frames after the key, the weapon entering `OPEN INVENTORY` 0.56 s after it as drawn; `local\shots\search_670.png`, `search_900.png`); `--equip --loot --drag 170:450:1150:250` then thirteen `--drag 1500:850:1500:850` and `--drag 1250:285:170:450` (the rifle taken off its slot under the open screen and put back: put away in 0.80 s, the empty hands, the draw, and `OPEN INVENTORY` six drawn frames after the weapon is at rest). A drag takes sixteen drawn frames and they follow one another from the ninetieth: a drag from a place onto itself is a wait.

## Added 2026-10-12, afternoon (doors, containers and a trunk; the range's things)

- **The recordings made by hand with recorder 0.6.1** are `local\traces\byhand2` (fourteen, 12:15 to 12:31, the user's own file names after the time: four doors, two pick-ups standing, a technical supply crate, a wooden crate on the floor, a dead Scav, a pick-up crouched, a car's trunk opened and shut, a drawer, a crate searched lying down). `byhand` over them is kept as `local\byhand2.txt`; `local\byhand2_changes.txt` is `changes.sh` over each with the thing's fields.
- **`bash local/changes.sh <trace> keys_down looking_at thing thing_state thing_gesture p_IsInteraction p_InteractionIndex inventory_open state thing_angle=`** is the short of a 0.6.1 recording by hand. `thing_gesture` is the game's number (the animator's plus seven for doors and containers); `thing_angle` creeps, so it is listed without starting a line.
- **`bash local/swing.sh <trace> <key time> <pace> [every] [wait]`**: a door's, lid's or trunk's angle frame by frame from the key beside the game's routine worked by hand (pace 1 and wait 0.5 for a door, 1.5 and 0.55 for a container or a trunk). `bin\byhand` now does the same with the model itself; the script is the quick look.
- **`target\debug\probe.exe bodystates - Player_anim_controller | grep interacts`**: the body states that let the player look at a thing.
- **`target\debug\probe.exe hardsettings DelayToOpen`** (and `LOOT_RAYCAST`, `DOOR_RAYCAST`, `BEHIND_CAST`, `DoorCurve`): the figures the things go by.
- **The range's things in captures:** `target\debug\trp.exe --start <place> --tap 200:f [--tap 500:tab] --log-frames <log> --screenshot <png> --screenshot-frame <n>`, with places `doors`, `yard` (the far side of the doors), `car`, `wooden-crate`, `supply-crate`, `scav`, `drawer`, `table`, `high-shelf`, `floor`; each turns the view to its thing unless `--yaw` or `--pitch` is given. `& .\local\things_shots.ps1 -Places 'drawer','car'` takes three captures of each (the prompt, the gesture, what it did) into `local\shots\things\`; the gestures as drawn: `awk '{ g = $(NF-2); if (g != last) { print $1, $3, g; last = g } }' local/things_<place>.txt`.
- **Tests:** `cargo test -p trp-sim --lib things` (the model: waits, swings, refusals, a door's gesture by side), `cargo test -p trp-sim --lib collision` (now a turned block), `cargo test -p trp-app range -- --nocapture` (the doors walked through, worked and swung onto a standing character; each container's gestures), `cargo test -p trp-sim --test install the_hands_work_a_container -- --nocapture` (now the gestures 16, 21, 27, 28 and the search at 0.55 s), `cargo test -p trp-sim --test install a_search_lying_down -- --nocapture`.
- **The regression is kept as `after30`**, identical to `after29` in every report but the time lines. 183 tests pass.
- Captures of the afternoon: `local\shots\things\door_400.png` (a door pulled open, gesture 19), `yard_560.png` (pushed from the far side, 20), `wooden-crate_150.png` (the prompt), `wooden-crate_560.png` (the lid open after the screen), `car_560.png` (the trunk open), `drawer_560.png` (the drawer out), `floor_260.png` (a pick-up from the floor, the stance going down).

## Added 2026-10-12, evening (the range laid out afresh, twice)

- **The range's layout changed** (`crates/trp-app/src/range.rs`, every position a named constant at its top). First spread out (the user: "too cramped together", no posts), then laid out as the user drew it on the overhead capture: the knee-high wall across the lane 5.6 m ahead with the airdrop crate against it; the door wall across 6 m behind with the doors in its middle; the things in a row 4.5 m behind it, shoulder to shoulder, the car at its right end; the stairs and platform to the left, the lower platform touching the knee-high wall's left end; the climb block off to the right behind; the ring 18 m out. Notes above that give places, distances or frames for walks about the range describe the old range (the high wall 5 m left, the low wall 5 m right, posts, the ring at 12 m, the car behind the start, the platform at 14 m).
- **What moved for the scratch scripts:** `local\scenarios-app\vault-wall.json` now goes over the knee-high wall with `--yaw 20` (logged: `Vault` 6.01 s into the run; `local\pace_vault_new.txt`), and finds no block after it; `local\to_crate.json` walked to the crate's old place and no longer reaches it; a walk into a high wall is `--yaw 180 --hold w` (the shut doors) or `--start doors --hold w`.
- **`target\debug\trp.exe --view map --screenshot <png> --screenshot-frame 120`** captures the range from 30 m overhead, the lane's way at the top; `--view map:<metres>` from that height over the character (with `--start`). Kept: `local\shots\range_before.png` (the range as it was), `range_after.png` (spread out), `range_drawn.png` (as the user drew it); the user's drawing itself was a picture in the chat.
- `cargo test -p trp-app range` checks the doors from the new side, every `--start` place clear of everything, the row's things half a metre apart or more, the lower platform at the knee-high wall's end, the airdrop crate against that wall, and the ring clear of everything. 184 tests pass. Captures of every thing from its place again (`& .\local\things_shots.ps1`): each gives its recorded gesture.


## Added 2026-10-12, 15:00 (the inventory's clicks and keys, recorder 0.6.2, the M3's scripts)

- **`target\debug\probe.exe keys <part of a name>`** prints the install's key bindings (`keys weapon`, `keys slot`, `keys inventory`) and two game settings; **`probe states <weapon id> parameters`** a weapon animator's parameters with the kind of each; **`probe scenes zzz`** (a filter that matches no scene) prints just the equipment panel and the quick-use panel as read. The M3's states are kept as `local\m3_states.txt` (778 lines; `grep -n "^\[" local/m3_states.txt` lists them).
- **Clicks in a capture run:** a `--drag` that ends where it begins is a click (the button goes down four frames into its sixteen and comes up at twelve), two in a row a double click, and `--tap <frame>:ctrl:<frames>` or `alt` holds the key over them. The drags begin at frame 90. What was run on 2026-10-12, in a 1600 by 900 window:
  - `--loot --bare --tap 88:alt:52 --drag 1100:180:1100:180 --drag 1385:200:1385:200 --drag 1120:360:1120:360` puts the crate's rifle, vest and backpack on (`local\shots\s31_f.png`);
  - the same and `--drag 700:700:700:700 --drag 806:150:806:150 --tap 160:4:3 --tap 168:ctrl:20 --drag 632:175:632:175 --drag 545:545:545:545 --drag 545:545:545:545 --tap 216:ctrl:20 --drag 738:518:697:719` binds the medical kit to `4`, sends a magazine to the crate, opens the backpack's window and lets a stack of 60 go into it with Ctrl held (`s31_c.png`, frame 300: the window asking 30 of 59); with `--tap 245:enter:2 --drag 1300:500:1300:500 --drag 590:572:900:300` after it the split is made and the window moved by its caption (`s31_d.png`, frame 330);
  - `--equip --tap 80:tab --drag 170:450:170:600 --tap 130:tab --tap 400:3 --tap 700:2 --tap 800:3 --log-frames local\frames_s31.txt` drags the rifle from the sling to the back and draws it with `3`; `awk '{ if ($5 != last) { print $1, $3, $5; last = $5 } }' local/frames_s31.txt` lists the animator's changes of state.
  A click's place moves with the layout: look at a capture first.
- **Tests:** `cargo test -p trp-sim --lib inventory` (the model: `a_thing_is_sent_to_the_other_side` and the three after it), `cargo test -p trp-sim --test install things_are_sent -- --nocapture` (the install's own items), `cargo test -p trp-app inventory_view` (the screen's clicks without a window). 192 tests pass.
- **The regression is kept as `after31`**, byte for byte `after30` in all fifteen reports; `local\byhand2_after31.txt` is `local\byhand2.txt`.
- **Recorder 0.6.2** is built into `local\oracle\stlkreft-oracle` (`local\recorder_build_062.txt`; the plugin before it is `local\Plugin_before_062.cs`). New in a trace: `equipment` and `loot` (each list as the header's `equipment`, on the frame either changed and on the first), `bound` (the quick-use keys by the game's names, `Item4` to `Item10`, each with its item's id), `in_hands` (the id of the item in the hands), `keys_up`, and with any key down or up `held` (Ctrl, Alt, Shift), `mouse` and `screen`; `fire_mode`; and `hp_<name>` for every float, whole number and flag of the weapon animator's parameters, each on the frame it changes (triggers are left out). `bin\byhand` does not read any of these yet.
- **The M3's scripts:** `hosts\spt-oracle\scenarios-m3` (70 to 88, 157 s of script) and `local\oracle\m3-only`, the plugin laid out with them alone. They are written for the stock five-shell tube and need the weapon set to pump action with the tube full and a shell chambered, and forty shells within reach; the script's `semiauto` press calls the weapon's own change of mode. Their traces go to `local\traces\recording16` with a `ground.txt`, and replay with `--scenarios hosts\spt-oracle\scenarios-m3` once the replay knows the weapon.
- **Scratch of the session** (in the session's own scratchpad, not kept): the edit specs. Kept under `local\`: `build_s31.txt`, `check_s31.txt`, `test_s31.txt`, `app_s31_*.txt`.
- **While the user has the app open** cargo cannot replace `trp.exe`: `cargo check -p trp-app`, `cargo test -p trp-app` (its test binary has another name) and everything in `trp-sim` and `trp-data` still work, and no second app is started.

## Added 2026-10-12, 17:00 (the sixteenth recording, the recordings made by hand with recorder 0.6.2, the M3)

- **The sixteenth recording** is `local\traces\recording16` (recorder 0.6.2, 2026-10-12 15:24 to 15:27; the nineteen scenarios of `hosts\spt-oracle\scenarios-m3`, 70 to 88; 18,864 frames; the stock M3 Super 90, mastering 0, field of view 74; level ground, its `ground.txt` is `0 0`). It keeps `86` and `88` as it was made with them in its own `scenarios` folder, the two having been rewritten since. Replayed with `target\debug\replay.exe --scenarios hosts\spt-oracle\scenarios-m3 --traces local\traces\recording16`: the replay takes the weapon from the traces' first line. `local/regress.sh`, `statecheck.sh`, `metricdiff.sh` and `metricgrew.sh` include it (`r16_replay_<name>.txt`). Kept: `local\r16_replay_first.txt` (the first run, the two fillings from empty doing nothing), `r16_replay_second.txt`.
- **The short of a scripted M3 trace:** `awk '/^[0-9][0-9]-m3/ {name=$1} /^  (hands_anim|hands_anim_time|hands_blending|hands_entering|machine_entering|hp_[A-Za-z]+|chamber|magazine|in_hand|aiming|fire_mode|p_ProneReload|p_ReloadFloat) / { if ($2+0 != 0) print name, $1, $2, $3, $4 }' local/r16_replay_after32.txt` prints every weapon value that is off anywhere, and prints nothing.
- **The regression is kept as `after32`.** `bash local/metricdiff.sh after32 after31` prints one line, `possible_tilt`, smaller in 108 scenarios. `metricdiff.sh` pairs two runs by recording, scenario and value now, and names what only one of them has.
- **The recordings made by hand with recorder 0.6.2** are `local\traces\byhand3` (sixteen, 15:29 to 15:47, the user's own names after the time). `target\debug\byhand.exe local\traces\byhand3` is kept as `local\byhand3.txt`; `grep "are the game's\|the state differs" local/byhand3.txt` is the short of it. A recording begun with the hands already down for the screen has the simulation's taken down before the first frame is compared.
- **`perl local/moves.pl <trace> [names]`**: the short of a 0.6.2 recording by hand without the model: every key down and up with what is held and where the mouse is, and every change of `equipment`, `loot`, `bound`, `in_hands`, the controller, the operation and the animator's state, the lists as items that came, went or moved. `local\byhand3_names.txt` names the templates met (made by a line of perl over `templates\items.json`); `local\byhand3_moves.txt` is it over all sixteen.
- **`perl local/weapon_events.pl local/m3_states.txt <trace> [trace ...]`**: what the weapon did in each recording, a line a trace: every change of its animator's state by name, of the operation in hand, the fire mode, the rounds, the bolt, the hammer and the sights, with its time (the states file is `probe states <weapon>` kept as text; a state's number in a trace is the low 24 bits of the CRC-32 of its own name). **Read it before saying what a scenario showed:** a scenario that replays with every frame the game's has still only shown what happened in it. `bash local/changes.sh <trace> operation fire_mode magazine chamber hp_BoltCatch hp_Armed hp_CanReload hp_AmmoInMag aiming hands_anim` is the same with numbers for states.
- **The M3 in the app:** `target\debug\trp.exe --weapon 6259b864ebedf17603599e88 --scenario hosts\spt-oracle\scenarios-m3\86-m3-reload-cut.json --log-frames local\frames_m3_86.txt --screenshot local\shots\m3_86_end.png --screenshot-frame 2300` plays a recorder's script through the app (at 165 drawn frames a second the script's 11 s end near frame 2300; the script begins some 3.9 s into the log's clock); `awk '{ if ($5 != last) { print $1, $3, $5, $6; last = $5 } }' local/frames_m3_86.txt` lists the animator's states. Kept: `local\shots\m3_86_end.png` (at rest, five and one), `m3_86_reload.png` (a shell on its way in).
- **The crate's weapon taken out with Alt and a click:** `--loot --bare --tap 88:alt:52 --drag 1100:180:1100:180 --tap 200:tab --log-frames ...` puts it on the sling; the hands stay empty until `2`, as the code has it for a slot never drawn from (`--tap <frame>:2`).
- **Tests:** `cargo test -p trp-sim --test install the_m3 -- --nocapture` (the M3 on the install's own controller: each case with the states it goes through and when), `cargo test -p trp-sim --lib inventory` (now also `a_tube_is_filled_from_the_pockets_and_the_vest` and `what_a_thing_sent_over_is_looked_for_room_in`). 195 tests pass (`local\test_s32.txt`).
- **Reading the game for a weapon:** `awk 'NR>=11600 && NR<=15740 && /^\t\t(public|protected|private|internal) .*\(/ {print NR": "$0}' local/decomp/src/EFT/Player.cs` lists the firearm controller's own methods with their lines; the operations are nested classes above it (`grep -n "class .*Operation" local/decomp/src/EFT/Player.cs`), and `GetOperationFactoryDelegates` says which a weapon gets. The marks' handlers are `IEventsConsumerOn...` and the `method_NN` each calls.
- **Scratch of the session** (kept under `local\`): the edit specs `edit_firearm_m3.txt`, `edit_replay_m3*.txt`, `edit_byhand_062.txt`, `edit_inv_*.txt`, `edit_app_*.txt`, `edit_*_spec.txt`, `edit_readme_s32*.txt`, `edit_agents_s32.txt`; `byhand_before_062.rs`, `README_before_s32.md`, `AGENTS_before_s32.md`; `append_m3_test.rs`.

## Added 2026-10-12, evening (the kick, the three new things, what was not run)

- **The kicks, beside the model:** `cargo run -p trp-sim --bin byhand -- local\traces\byhand3 Breach` (kept whole as `local\byhand3.txt`, which is the new run: only the two kicks' lines differ from the afternoon's; `local\byhand2_s33.txt` is the same as `local\byhand2.txt`). A kick is told from an ordinary key by the movement state going to `Approach` and then `BreachDoor`; it lands on the first frame the body's animator is 0.45 through the kick's state and past its blend in (`body_anim_time`, `body_blending`).
- **A recorded kick by the frame:** the perl one-liners of this session print `t`, `state`, `pos`, `look`, `body_yaw`, `working_snap`, `thing_state`, `thing_angle`, `thing_from_player`, `body_anim_time`, `body_next_time` and `stamina` from a trace's lines by regular expression; frames 119 to 132 of "Breach unlocked door" are the approach, 150 the kick's state, 222 the kick, 282 the door open.
- **Tests:** `cargo test -p trp-sim --test install -- a_door --nocapture` (the kick with the recorded doors' places and the recordings' figures in its comments; the menu), `the_m3_s_stock`, `the_model_700`, `the_five_seven`, `the_four_weapons` (each weapon's motion and logic read, the crate holding them, the slot each is put on to, the menu's words in the install). 201 tests in the workspace.
- **The regression is kept as `after33`**, the same as `after32` in every figure (`for n in ...; diff local/r${n}_replay_after32.txt local/r${n}_replay_after33.txt`: one line each, the count of the body's clips read).
- **The new weapons with the probe:** `probe states <id>` and `probe states <id> parameters` are kept as `local\57_states.txt`, `57_params.txt`, `m700_states.txt`, `m700_params.txt`; an item's template properties with `probe json templates/items.json /<id>/_props 1` (kept as `local\props_<id>.txt`); the body's states for a kick as `local\body_kick.txt` and `body_approach.txt`.
- **Not run, to be run first next time** (the user's app was open): `cargo build -p trp-app`; `--loot --bare` for the crate and its bar; `--weapon <each id> --hold fire` and `--hold reload`; `--start doors` for the menu (a wheel notch for `--tap` does not exist yet: the menu's second line cannot be reached in a capture until it does); Alt+`L` needs `--tap` to take `l` and to hold `alt` with it.
- **Scratch of the session** (under `local\`): `notes_s33.md` (what was read, before the specs); the edit specs `edit_*_s33*.txt`; `append_tests_s33*.rs`; the files as they stood before, `*_before_s33.*`; `weapon_tail_s33.rs`, `agents_middle_s33.md`.

## Added 2026-10-12, night (the app's new code run; the sights; a part on its bone)

- **Captures that run the evening's code** (each from `E:\stlkreft\TRP`, with `target\debug\trp.exe`, never two at once; kept as `local\shots\s34_*.png`):
  - the crate, and scrolled: `--loot --bare --screenshot <png>`; `--loot --bare --wheel 200:-12 --screenshot <png> --screenshot-frame 260`;
  - a weapon out of the crate and drawn: `--loot --bare --drag 1060:258:400:458 --tap 130:tab --tap 220:1 --screenshot <png> --screenshot-frame 620` (the Five-seveN to the holster, key `1`); `--drag 1250:258:170:458 ... --tap 220:2` (the Model 700 to the sling);
  - the menu and a kick: `--start yard --wheel 250:-1 --tap 280:f --screenshot <png> --screenshot-frame <n>`: a frame before 280 has the menu, one near 400 the kick, one past 560 the door open; `--start doors` with the same is the kick that fails; add `--log-pacing local\pace_x.txt` and `awk 'NR>1 { key=$16" "$(NF-1)" "$NF; if (key!=last) { print $1, $2, key, "feet", $18, $20; last=key } }' local/pace_x.txt` lists the movement state and the body's animator's state as they change;
  - the key and the lock: `--start yard --loot --bare --drag 1330:415:545:430 --drag 1517:362:545:430 --tap 140:tab` puts the backpack on and the key in it and shuts the screen; then frame 250 has `UNLOCK` first in the menu, `--tap 260:f` unlocks, `--tap 640:f` opens;
  - the M3's stock: `--weapon 6259b864ebedf17603599e88 --hold fold` (and `fold+aim`), with `--view side` from outside.
  The places in the window are the crate's as it is laid out now (the AKM and the M3 on its first row, the Five-seveN and the Model 700 on its second, the vest and the backpack under them, the key at the right, under the rounds).
- **New options of the app for such runs:** `--wheel <frame>:<notches>`, `--hold fold`, `--weapon-skill <0..51>`.
- **`local\crop.ps1 <in> <out> <x> <y> <w> <h> <scale>`** cuts a rectangle out of a capture and enlarges it (paths from the workspace): `powershell -File local\crop.ps1 local\shots\s34_m3_side2.png local\shots\x.png 560 300 520 200 3`. It is how the M3's pistol grip was seen.
- **A weapon's bones at rest:** `target\debug\probe.exe weapon <id> <part of a path>` prints each bone whose path has the word with its place and turn in its parent (an empty word from bash prints all: kept before and after the mounting rule as `local\nodes_before_s34_<id>.txt` and `nodes_after_s34_<id>.txt`). `probe scripts "<bundle key>"` lists the scripts on a part (a `ModPlacer` among them or not); `probe clips <id> <clip> tracks <word> [time]` a clip's curves for the bones whose path ends in the word; `probe clips <id> idle check` the idle clip against the rest pose.
- **The regression is kept as `after35`.** `after34` against `after33` is the sights' rule (`bash local/metricdiff.sh after34 after33 '^(eye|camera_pos|weapon_seen)$'`); `after35` against `after34` the mounting rule (kept: `local\metricdiff_after35_after34.txt`, nine lines, all of them bones' turns).
- **Tests:** `cargo test -p trp-sim --test install -- the_eye_is_lined a_weapon_skill a_part_goes --nocapture` (the sights' line and the eye's shift on each weapon; the draw and the reload at level 0 and 50; where the M3's grip lies and each idle clip against the rest pose). 204 tests in the workspace (`local\test_s34.txt`).
- **Scratch of the session** (under `local\`): the edit specs `edit_*_s34*.txt`, `append_tests_s34*.rs`, `append_*_s34.md`, `agents_middle_s34.md`; the files as they stood before, `*_before_s34*`; the runs' output `run_s34_*.txt`.


## Added 2026-10-12, late night (the options tab; captures that press keys and click buttons)

- **The tab by capture.** The tabs' row is 78 pixels down in a window of 1600 by 900; `OPTIONS` is at 1490. Its three parts' buttons are at 92, 248 and 404 across, 115 down. So: `--inventory --bare --drag 1490:78:1490:78` opens it; add `--drag 248:115:248:115` for the graphics, `--drag 404:115:404:115` for the features (and `--drag 40:172:40:172` after it throws the switch of the first-person hands), `--wheel 130:-12` to scroll the bindings. A row of the bindings is 26 pixels (the first at 243 down; `Prone` at 581), its first key cell 450 across.
- **A key chosen by capture:** a click on the cell, then a press: `--drag 1490:78:1490:78 --drag 450:581:450:581 --tap 130:r:10` puts `Prone` on `R`. The first drag begins at frame 90 and each takes sixteen frames, so a second drag's click is over by frame 120.
- **The display by capture:** `--drag 395:146:395:146` (VSync off), `--drag 384:175:384:175` (the field of view's bar at 60), `--drag 219:260:219:260` (1366 by 768). The picture comes out at the new size. **Do not click `FULLSCREEN` or `BORDERLESS` in a capture: it changes the user's screen.**
- **The character's keys by capture** (a run with `--tap` and no `--hold`; `--log-pacing` has the movement state in its sixteenth column, `--log-frames` the weapon's animator's state in its fifth): `--equip --yaw 90 --tap 60:w:900 --tap 200:shift:10 --tap 420:shift:10 --log-pacing local\x.txt --screenshot local\shots\x.png --screenshot-frame 640`, then `awk 'NR>1 { if ($16 != last) { print $1, $16; last = $16 } }' local/x.txt`. `--tap` takes a letter, a digit, `tab`, `delete`, `space`, `ctrl`, `alt`, `shift`, `caps`, `enter`, `escape`, `lmb`, `rmb`.
- **`probe keys <part of a name>`** now prints a variant's wheel, the axes' pairs, the display's settings and the double click's time; `probe keys ""` prints everything.
- **A run someone is at the keys of reads and writes `local\options.json`**; delete it to be back at the install's keys and the app's own window. A capture never reads it.
- **Tests:** `cargo test -p trp-sim --lib bindings` (the manager's nine); `cargo test -p trp-sim --test install commands_do` (the simulation's commands); `cargo test -p trp-app` (the key names, the options file). 217 in the workspace (`local\test_after36.txt`).
- **The regression** is `after36`, byte for byte `after35` (`cmp local/r<N>_replay_after36.txt local/r<N>_replay_after35.txt`).
- **`local\crop.ps1 <in> <out> <x> <y> <w> <h> <scale>`** was written again this session (overwritten by mistake); it takes the same seven, the last five optional.
- **Scratch of the session** (under `local\`): `shots\opt*.png`, `app_opt*.txt`, `opt_pace_*.txt`, `opt_frames_d.txt`, `agents_middle_s36.md`, `AGENTS_before_s36.md`, `history_s36.md`, `findings_s36.md`, `workbench_s36.md`, `agents_swap.pl`.

## Added 2026-10-12, night into the 13th (the rest of the keys; the pointer measured)

- **The recorded keys beside the model:** `target\debug\byhand.exe local\traces\byhand4` (and `byhand3`); the lines that begin `keys:` are the bindings the manager gives, each with how long after its key and, for a reload, the inventory and the fire mode, the game's own frame. `grep "keys: of"` is the short of it.
- **`local\traces\byhand4`**: two recordings of 2026-10-12, 21:12 and 21:33 (recorder 0.6.2): `R` tapped with the M3 in hand, and 1.2 s with no key.
- **`probe keys <part>`** now also prints the four mouse settings of `Control.ini`, the two inverted axes, the build's own mouse axes (`Mouse X`, `Mouse Y`, `Mouse Scroll`, each with what a count is multiplied by) and each turn axis's binding factor. **`probe bodystates <part>`** prints a state's marks (`0.250 OnBackpackDrop`). **`probe screen level44 "ControlsPart" 7`** is the game's controls tab.
- **Captures of the night** (1600 by 900): the tab `--inventory --bare --drag 1490:78:1490:78`; a bar slid `--drag 80:836:200:836` (the sensitivity's bar runs from 68 to 308 across, 836 down; the next begin at 455, 843 and 1230); the inverted Y axis `--drag 838:863:838:863`; the list to its end `--wheel 150:-40`; `EXIT` `--drag 1507:115:1507:115` (the run ends with code 0 and writes no picture). The keys: `--equip --tap 200:alt:260 --tap 230:d:60` (the slow lean), `--equip --yaw 90 --tap 60:w:900 --tap 300:caps:10` (the walk key), `--equip --tap 200:z:5 --tap 215:z:5` (the pack; `--log-frames` has the gesture's state in its last columns), `--tap 200:l:5`, `--tap 200:alt:40 --tap 215:b:5`, `--tap 200:b:5 --tap 215:b:5`, `--tap 200:r:7`.
- **`local\cursor_test_s40.ps1 [-AltMenu] [-Seconds 2]`** (PowerShell; renamed `window_keys_test.ps1` the next morning, when it was made to watch the keys too: the section below): runs the app as someone at the keys does, finds its window by its title, brings it to the front, sends `W` thirty times a second with Alt held as a keyboard does (only while that window is in front) and counts every showing and hiding of the pointer; ends the app by its own number. `-AltMenu` runs the app with `--alt-menu`. It takes the mouse for some twelve seconds and reads `local\options.json`. Its last output is in the settings spec.
- **Tests:** `cargo test -p trp-sim --test install a_lean_is_made a_weapon_is_looked` (the night's two); `cargo test -p trp-app` (the wheel's notch, the bars, the options file with the mouse in it). 222 in the workspace (`local\test_after37.txt`).
- **The regression is kept as `after37`**: fifteen reports byte for byte `after35`'s; `r8` differs in its second and third lines, which time the reading of the clips (`cmp` says "differ": look with `diff` before believing it).
- **Scratch of the session** (under `local\`): `edit_s40_*.txt` (the edit specs), `append_s40_*.rs` and `.md`, `options_view_before_s40.rs`, `README_before_s40.md`, `shots\s40_*.png`, `app_s40_*.txt`, `s40_frames_*.txt`, `cursor_test_out.txt` and `_err.txt`.

## Added 2026-10-13, early (keys through the real window; the guide's captures)

- **`local\window_keys_test.ps1 [-AltMenu] [-Seconds 2] [-Folder <where to run from>]`** (PowerShell; it was `cursor_test_s40.ps1`): runs the app as someone at the keys does with `--log-pacing`, finds its window by its title, brings it to the front, sends `W` thirty times a second with Alt held as a keyboard does (only while that window is in front), counts every showing and hiding of the pointer, asks the window to shut, and says how far the feet went. **Run it after anything that touches the window or the keys' way in.** With `-Folder` set to a folder that has only `local\eft_dir.txt` (the session's was in the scratchpad) the app has no options file: a window of 1600 by 900 with a title bar, which is where the pointer's fault shows, and the workspace's `local\options.json` left alone. It takes the mouse for some twelve seconds.
- **Captures of the guide:** `--equip --screenshot <png> --screenshot-frame 330` (the two panels); with `--inventory` (the guide stands back, the debug row over the tabs); with `--view side` (the free camera's keys); with `--tap 200:f1:5` (hidden). `--tap` now knows `f1`.
- **Tests:** 222 in the workspace (`local\test_after38.txt`). The simulation was not touched: the regression stands as `after37`.
- **Scratch of the session** (under `local\`): `edit_s41_*.txt`, `append_s41_*.md`, `guide_fn_s41.rs`, `keys_before_s41.rs`, `keys_new_s41.rs`, `hud_before_s41.rs`, `main_before_s41.rs`, `shots\s41_*.png`, `app_s41_*.txt`, `cursor_test_pace.txt`.

## Added 2026-10-13, later (the controls' list in the middle; Escape)

- **The options' controls by capture, as they now lie** (1600 by 900; the list was narrower for an hour and is the page's width again): the list is 18 to 1582 across; its first row (`MOVE RIGHT`) is 214 down and a row is 26 pixels, with 8 more before each group (the first after `PREVIOUS`); the first key's cells are at about 750 across, the second's at 1100, the way of being pressed at 1400; `QUIT` is at 1507, 115. The bars are 240 wide and begin at 68, 455, 843 and 1230 across, 836 down.
- **The tab the screen opens on, by capture:** `--equip --inventory --start drawer --drag 1490:78:1490:78 --tap 150:tab:5 --tap 230:f:5` (the options tab shown, the screen shut, the drawer searched: the gear tab, the drawer beside it); the same with `--tap 230:tab:5` and no `--start` (the key: the gear tab).
- **Escape by capture:** `--equip --inventory --tap 200:escape:5` (the screen shut); `--equip --inventory --drag 545:175:545:175 --drag 545:175:545:175 --tap 200:escape:5 --tap 260:escape:5` (two clicks on the vest open its window; the first press shuts the window, the second the screen); `--inventory --bare --drag 1490:78:1490:78 --drag 750:225:750:225 --tap 200:escape:5` (a key's cell clicked on the options tab and the choosing dropped: "kept as it was", the screen still open).
- **Scratch of the session** (under `local\`): `edit_s42_*.txt`, `controls_part_s42.rs`, `options_view_before_s42.rs`, `options_view_new_s42.rs`, `shots\s42_*.png`, `app_s42_*.txt`.

## Added 2026-10-13, a new chat (the StG 44: looking at a model, choosing a donor, checking the build)

- **The model**: `probe fbx <file>` (its models and their pieces), `probe fbx <file> records [depth]`, `probe fbx <file> view <png> [pixels a centimetre] [ahead behind above below]` (a window drawn at 24 or more to a centimetre has a line every centimetre), `probe fbx <file> slice <model> <x|y|z> <from> <to> [piece|x] [points]`. The grid's origin is the model's own.
- **A donor**: `cargo run -p trp-sim --bin donor -- [--no-catch] [--detail] [--from-bone] [names or ids]`; with no name, all 91 (13 s). `local\donor_table_s42.awk` cuts its output down to a few lines a weapon. `donor --overlay <weapon> <png> [--model <fbx> --place <ahead> <up> <right>] [--moment idle|draw:<s>|reload:<s>] [--scale n] [--window a b c d] [--without <model>]... [--no-catch]` draws the weapon as its animator has it with both hands' bones, each hand at its palm's marker, and prints the hands' joints and the weapon's bones from the weapon's own bone. **Send its output to a file, not to `head`.**
- **The weapon built**: `probe custom` and `probe custom 66f0a5740000000000000044` (its parts, weight, pictures); `probe item 66f0a5740000000000000044`; `probe weapon 66f0a5740000000000000044 Weapon_root` (its rig as edited); `donor --overlay 66f0a5740000000000000044 <png> --moment reload:3.5`.
- **In the app**: `--weapon 66f0a5740000000000000044`, with `--view side`, `--hold aim`, `--hold reload --log-frames <file>` (in this run the reload's key is heard at about frame 400 of a capture and the magazine is in hand at 420, the new one going in at 700), and out of the crate: `--weapon 66f0a5740000000000000044 --loot --bare --tap 88:alt:52 --drag 1100:180:1100:180 --tap 200:tab --tap 260:2 --screenshot <png> --screenshot-frame 425` (the action being worked).
- **Kept from this session** (`local\`, suffix `_s42`): `donor_all_s42.txt` (all 91 as the install has them) and `donor_all_nocatch_s42.txt` (as weapons with no catch), `donor_hands_s42.txt` (one line a weapon: the hand in a draw and in a reload), `donor_mk16_detail_s42.txt` (the SCAR-L's draw and reload every twentieth of a second), `mk16_props_s42.txt` and `akm_props_s42.txt` (the two templates); in `local\shots`: `stg44_model_s42.png` and `stg44_receiver_s42.png` (the model), `stg44_over_*_s42.png` (over six weapons, laid by the trigger meshes: the wrong way), `stg44_mk16_fit_s42.png` (laid by the forefinger), `stg44_idle_s42.png` and `stg44_reload_*_s42.png` (the weapon built, at rest and through a reload), `stg44_app*_s42.png` (captures of the app; `_metal_` with the pictures' metal, which is what was kept). `README_before_s42.md` and `AGENTS_before_s42.md` are the two files as the session found them.
- **The regression** of this session is `after42` (`bash local/regress.sh after42`, then `cmp` of each `local\r<N>_replay_after42.txt` with `after37`'s: the same but for `r8`'s two lines of reading times).

## Added 2026-10-13, midday (a character seen from outside; recorder 0.7.0)

- **Looking at a prefab's script**: `cargo run -p trp-data --bin probe -- script <bundle key> <class name> [depth] [items]` prints every script of that class in the bundle as stored, with the object it is on (`probe script assets/content/commonprefabs/playersuperior.bundle ProceduralWeaponAnimation 5 40`; `PlayerBones` likewise). A reference it shows (`PPtr(file 0, path N)`) is followed with `probe dump <bundle key> N`.
- **The body animator's layers and what switches them**: `probe bodystates - Player_anim_controller tree` (every state, 2,059 lines; `Weapon.`, `UtilityOperation.` or `Additive_FullBody` as the filter for one layer), `probe bodystates zzzz Player_anim_controller parameters` (the parameters alone).
- **The tests**: `cargo test -p trp-sim --test third_person -- --nocapture` (seven, 8 s; they print the figures the spec quotes).
- **In the app**: `F5` while playing (watched from three metres off; `F4` then flies the camera, `F5` again plays from where it was left). Captures: `--view body` and `--view side` are now the third-person pose (`--pov first` gives the old picture, the first-person rig from outside); `--view watch --tap <frame>:<key>` presses keys through the bindings while watched (`--view watch --yaw 90 --tap 60:w:900 --tap 200:shift:10 --screenshot-frame 420 --screenshot local\shots\x.png` is a sprint; `--tap 100:rmb` the sights; `--tap 100:x` lying down). `--view body --hold reload --screenshot-frame 520` has the torso turned to the vest.
- **Recorder 0.7.0** (`local\oracle\stlkreft-oracle`, built with the usual line; `local\Plugin_before_070.cs` is 0.6.2's source): **`\` switches the player to how others see it and back** (the setting `PointOfViewKey`), between recordings or in one made by hand. The picture is then the inside of the head: the game has no camera for it. A third-person recording: `\`, then `[` with the scripts wanted, then `\` again. Every frame has `third_person`; the animator's `p_ThirdPerson` and `p_FirstAction`; `neck` and `head` in the player's axes.
- **Kept from this session** (`local\`, suffix `_s43`): `shots\tp_*_s43.png` (the captures the spec names), `app_tp*_s43.txt` (their runs' output), `window_keys_test_s43.txt`, `oracle_build_s43.txt`, `regress_after43_s43.txt`.
- **The regression** of this session is `after43` (`bash local/regress.sh after43`; each `local\r<N>_replay_after43.txt` set beside `after42`'s with `cmp` and `diff`: the same but for the lines that say how many clips were read and how long that took, 517 for 508).

## Added 2026-10-13, afternoon (the free look; the weapons counted)

- **The free look's tests**: `cargo test -p trp-sim --test free_look -- --nocapture` (two against the install), `cargo test -p trp-sim --lib head_look` and `the_mouse_is` (the head's own three and the mouse's split).
- **In the app**: hold the middle mouse button and move the mouse; `F5` shows the head turning from outside. Captures: `--tap 100:mmb:300 --mouse 110:40:6:-2 --screenshot-frame 300` (the head held 30 degrees right and 10 up), the same with `--tap 100:mmb:150 --screenshot-frame 330` (let go, and back), and with `--view watch` (from outside). A head cut out of a capture and enlarged, to see which way it is turned: the lines with `System.Drawing` in this session's history (`local\shots\*_head.png`).
- **Every weapon of the database**: `cargo run -p trp-data --bin probe -- weapons [part of a name]` (a minute for all of them): one line a weapon with its class, its animator's states and clips, the two marks and its layers. Distinct sets of clips: `... | awk '!/not read/{print $11}' | sort -u | wc -l`; of states, `$9`. The tally by how they reload was a few lines of perl over `templates/items.json` (`ReloadMode`, `BoltAction`, `weapClass`), not kept.
- **The regression** after the free look is `after44` (the same as `after43` but for the two lines of reading times where they differ).
- **Recorder 0.7.0 was built a second time the same afternoon** with the free look in it: `head_rot`, `mouse_look`, `head_looking` and `mouse_axes` (the engine's `Mouse X` and `Mouse Y` as each frame read them) on every frame. The folder to copy is `local\oracle\stlkreft-oracle` as it stands after 13:23; a copy made before that lacks them.

## Added 2026-10-13, evening (the five kinds, the donors' catalogue, the documents)

- **The five kinds' tests**: `cargo test -p trp-sim --test kinds -- --nocapture --test-threads=1` prints, for each of the five, the animator's states as they are entered with the seconds since the key, and the rounds after each step. The ids are the constants at the file's top (`SV98`, `SKS`, `RHINO`, `MP43`, `MSGL`).
- **A weapon's animator, to read before writing its logic**: `cargo run -q -p trp-data --bin probe -- states <id> > file` (all of it; a part of a state's name as a third word narrows it, `parameters` lists the parameters with their kinds). The transitions out of `Hands.IDLE` are the first thing to read: they say what the animator has to be told for each thing to begin.
- **The five in the app**: `target\debug\trp.exe --weapon <id> --tap 200:lmb:6 --tap 420:r:6 --screenshot local\shots\x.png --screenshot-frame 700` is the weapon in hand, one shot, and the reload caught a second and a half in (kept: `local\shots\s43_{rhino,mp43,msgl,sv98,sks}_reload.png`). The crate with all ten: `--loot --bare --screenshot-frame 160` (`s43_crate.png`); where each lies there: the AKM 1147,178, the M3 1412,178, the Five-seveN 1066,258, the Model 700 1252,258, the Rhino 1437,258, the SV-98 1174,336, the M32A1 1437,336, the SKS 1147,416, the MP-43 1174,470, the StG 44 1147,550. One taken out and drawn: `--loot --bare --tap 88:alt:52 --drag 1174:470:1174:470 --tap 200:tab --tap 260:2 --screenshot-frame 560` (the MP-43 on the sling by Alt and a click, the screen shut, `2`).
- **The donors' catalogue**: `cargo run -q -p trp-sim --bin donor -- --catalogue` (a minute; writes `local\donors\catalogue.tsv`, tab-separated, its first line the columns' names, and lists what it could not measure); `cargo run -q -p trp-sim --bin donor -- --rank custom\stg44.json` reads it. A column by its number: `cut -f2,22-31 local/donors/catalogue.tsv` is each weapon with the hand, the part, the travel and the palm of its draw and of its reload.
- **A figure of an item without a tool for it**: `awk -v id=<id> 'index($0, "\"_id\": \"" id "\"") {f=1} f && /"ReloadMode"/ {print; exit}' /e/spt/SPT_Runtime/SPT_Data/database/templates/items.json` (the file is read, never written).
- **The regression** after the five kinds is `after45` (`bash local/metricdiff.sh after45 after44` prints nothing: no value of any scenario moved).
- **The documents as they stood before they were shortened**: `local\AGENTS_before_s43.md` and `local\README_before_s43.md`. What left the README is whole in `docs\status.md` and `docs\manual.md`.

## Added 2026-10-13, night (the Sunrise suit)

- **Where the other game is, for this machine**: `local\gamma_dir.txt` (`E:\GAMMAEFT`) and `local\anomaly_dir.txt` (`E:\AnomalyEFT`), one line each. Written on 2026-10-13; nothing is ever written to those folders.
- **Finding a thing of the other game's**: its item's section by its name in the texts (`gamedata\configs\text\eng\st_items_outfit.xml` of the text overhaul mod: `stalker_outfit_name` is `"Sunrise" Stalker Suit`), then `actor_visual` in a config that has the section loose; then the file under `mods\*\gamedata\meshes\`. Many base sections are only in the packed `db\configs\configs.db0`: a mod's optional file may still have them to read.
- **The two skeletons**: `target\debug\probe.exe ogf actors\stalker_neutral\stalker_neutral_2.ogf` (the model: its meshes, where each picture was found, its bones at rest beside the vertices that go with them) and `probe skin assets/content/characters/character/prefabs/tshirt_bear_turtleneck.bundle` (also `pants_bear_srvv`, `bear_head_0`): Tarkov's bones at rest.
- **The fit's tests**: `cargo test -p trp-data --lib -- outfit:: xray::` (seven, needing nothing) and `cargo test -p trp-data --test install an_outfit -- --nocapture` (the two games' own files: counts, bounds, the hands).
- **In the app**: `--wear 66f0a5740000000000000101` starts with the suit on. Kept captures (`local\shots\s43_sunrise_*.png`): `--wear <id> --bare --view side` (close, empty hands), `--wear <id> --view body` (armed, with Tarkov's kit over it), `--wear <id>` and `--wear <id> --pitch 65` (first person, and looking down), `--wear <id> --view watch --yaw 90 --tap 60:w:900 --tap 200:shift:10 --screenshot-frame 520` (a sprint), `--tap 120:c:8` (crouched). From the crate: `--loot --bare --view watch --tap 88:alt:30 --drag 1093:680:1093:680 --tap 240:tab --screenshot-frame 460` (Alt and a click on the suit, the screen shut, seen from outside); with `--drag 248:318:1040:630` after the click it is dragged back from the armour's slot and the character's own body is back.
- **The engine's published source**, where a question of the format comes up again: `https://raw.githubusercontent.com/OpenXRay/xray-16/dev/src/` and then `xrCore/FMesh.hpp`, `Layers/xrRender/SkeletonX.cpp`, `Layers/xrRender/SkeletonCustom.cpp`, `xrCore/Animation/Bone.cpp`, `utils/xrMiscMath/matrix.cpp`, `xrCore/LocatorAPI.cpp` (the packed archives). `curl` fetches them; nothing of them is kept in the workspace.
- **An archive's chunks**, to look before writing a reader for them: a script of a dozen lines that reads a number and a length and skips the body (chunk 0 is half a gigabyte) shows chunk 666, chunk 0 and the packed chunk 1.
- **A detail of a capture, enlarged** (the hands, a seam): `powershell -NoProfile -ExecutionPolicy Bypass -File local\crop_shot_s43.ps1 <in.png> <out.png> <x> <y> <width> <height> [times]` cuts a part out and enlarges it (four times unless told). Kept: `local\shots\s43_zoom_hands_bare.png` and `s43_zoom_hands_armed.png`, the suit's fingers after the fit was made again.
- **The suit on its own build** (the second fit; `local\shots\s43_sunrise2_*.png`): `--wear <id> --bare --view side` and `--view body`, `--wear <id> --view side` (armed), `--wear <id>` (first person), and with `--bare --view watch`: `--yaw 90 --tap 60:w:900 --tap 200:shift:10 --screenshot-frame 520` (a sprint), `--tap 120:c:8 --screenshot-frame 420` (crouched), `--tap 120:x:8 --screenshot-frame 520` (lying down).
- **The suit's arms for first person, looked at from outside**: `EFT_OUTFIT_ARMS=always` (an environment variable, Git Bash: `EFT_OUTFIT_ARMS=always target/debug/trp.exe --wear 66f0a5740000000000000101 --pov first --view side ...`) shows those arms in every view and leaves the upper body a shadow in every view: the rig as the character's own eyes have it, seen from beside. The same run without `--wear` and without the variable is Tarkov's own arms in the same pose, to set beside it.
- **First person without the panels**: `--fps off --tap 30:f1:4` hides the frame-rate readout and the guide. Kept (`local\shots\s43_arms_*.png`): `fp_akm` and `fp_akm_theirs` (the suit's arms and Tarkov's own, the same frame), `ads` (`--tap 120:rmb:6 --screenshot-frame 260`), `pistol` (`--weapon 5d3eb3b0a4b93615055e84d2`), `m3`, `reload` and `reload3` (`--tap 200:lmb:6 --tap 320:r:6`, frames 352 and 420: the left hand at the magazine), `side` (the variable above), `outside` (`--view side`: the body's own arms, as before).
- **Which frames a state is on**, before asking for a capture of it: the same run with `--log-frames <file> --screenshot-frame 900`, then `awk '{print $5}' <file> | uniq -c` lists the hands' states in order with how many frames each had.
- **A bone's own geometry**: `probe ogf <model>` ends each bone's line with how far its vertices reach along the bone from its joint and how far across, in centimetres: a finger's true length, where a hand's joint lies in the palm.
- **The arms' fit, measured**: `cargo test -p trp-data --test install an_outfit_s_arms -- --nocapture` prints each bone's length on the model, on its rig and on Tarkov's skeleton, and every joint of the left hand's distance from Tarkov's own.
- **Two sets of arms on the same frame**: the same command twice, once with `--wear <the suit>` and once without, with the same `--tap` and `--screenshot-frame`. For the look all over the weapon: `--fps off --tap 30:f1:4 --tap 200:l:6 --screenshot-frame 450` (the state `LOOK` is on frames 201 to 752; kept at 330, 450 and 570 as `local\shots\s43_look_<frame>_suit.png` and `_theirs.png`).

## Added 2026-10-13, late (the look measured; hands set side by side)

```
cargo run -q -p trp-data --bin probe -- clips 59d6088586f774275f37482f akm_look path weapon_L_IK_marker 2.55 2.85 0.004
        (a bone followed through a clip from the clip alone: its place, its speed, how fast it turns; `series` prints the tracks' own figures)
cargo run -q -p trp-data --bin probe -- texstats assets/content/hands/hands_bear_telnik/hands_bear_telnik.skin.bundle _d local/shots/hands_s43
        (a bundle's pictures written out to look at)
target\debug\trp.exe --wear 66f0a5740000000000000101 --play akm_look --at 2.0 --fps off --tap 30:f1:4 --screenshot local\shots\a.png --screenshot-frame 300
        (the look held still two seconds in; the same with ...102 is Tarkov's bare hands in the same pose, ...103 the SEVA's)
target\debug\trp.exe --wear 66f0a5740000000000000101 --hold w+shift --yaw 75 --pitch 88 --fps off --tap 30:f1:4 --screenshot local\shots\b.png --screenshot-frame 260
        (a sprint with the view straight down, toward the range's open side: what the character's own eyes see of its body)
target\debug\trp.exe --loot --bare --fps off --drag 1300:400:1300:400 --wheel 150:-12 --screenshot local\shots\c.png --screenshot-frame 320
        (the crate scrolled by the engine's own wheel events, the mouse put over its panel first)
cargo test -p trp-data --test install outfit -- --nocapture        (the three outfits built; the arms' size and the hands' skin at stations; the waist)
```

- **The look's stutter in a frame log**: `awk '$1>=630 && $1<=700 {if (p) printf "%d dt %.1f clip %s  L %6.1f %6.1f %6.1f\n", $1, ($3-pt)*1000, $6, ($14-a)*1e4, ($15-b)*1e4, ($16-c)*1e4; p=1; pt=$3; a=$14; b=$15; c=$16}' <log>` prints the left palm's travel a drawn frame in tenths of a millimetre: a column that climbs for five or six frames and snaps back, at 165 frames a second, is 30 a second. The same over all ten weapons was a loop of runs with `--weapon <id> --tap 200:l:4 --log-frames`, each followed by the animator's states as drawn and the second differences of the two palms and the weapon's moved bone (a scratch script; the figures are in `history.md`).
- **A capture's frames are not the same moments in two runs**; `--play <clip> --at <seconds>` is, and is what to use to set two things beside one another in one pose.
- **From Git Bash an argument that begins with `/` is rewritten as a Windows path** (`/weapon` became `C:/Program Files/Git/weapon` and matched nothing): give the probe a word that does not begin with one.
- `local\crop_shot_s43.ps1 <in> <out> <x> <y> <w> <h> [times]` still enlarges a part of a capture; this session's captures are `local\shots\s43_look_10{1,2}_{1.0,2.0}.png`, `s43_down_{sunrise,own}.png`, `s43_seva_{side,look}.png`, `s43_scroll.png`, and the hands' pictures in `local\shots\hands_s43\`.

## Added 2026-10-13, later still (the look drawn smooth)

```
cargo test -p trp-sim --test look -- --nocapture       (the AKM's look on its own animator: the pose bit for bit with and without a pose for drawing; the left hand's marker as the engine has it and as drawn)
cargo test -p trp-data --lib anim                      (the curve through a clip's samples: no corners, no overshoot, keys at every sample taken as samples)
target\debug\trp.exe --weapon <id> --fps off --tap 30:f1:4 --tap 200:l:4 --log-frames local\look.txt --screenshot local\shots\x.png --screenshot-frame 900
        (a look in the app, one line a drawn frame; fields 14 to 16 are the left palm as the camera sees it)
```

- **A stutter in such a log, as figures**: `awk '$1>200 && $1<=880 { for (c=0;c<3;c++) l[c]=$(14+c); if (n>1) { j=0; for (c=0;c<3;c++) { d=l[c]-2*pl[c]+ppl[c]; j+=d*d } s+=sqrt(j); k++ } for (c=0;c<3;c++) { ppl[c]=pl[c]; pl[c]=l[c] } n++ } END { print 1000*s/k }' <log>` is the left palm's mean change of travel a frame, in millimetres (0.24 for the AKM's look on the engine's lines, 0.16 drawn smooth). Its worst frame is no measure where the frames are uneven (`findings.md`).
- **The kept regression is now `after46`** (`after45`'s in every figure: the animator's second pose is read by nothing of the simulation).

## Added 2026-10-13, night (the suits' hands measured again; the switch)

```
target\debug\trp.exe --wear 66f0a5740000000000000101 --play akm_look --at 1.5 --fps off --tap 30:f1:4 --screenshot local\shots\a.png --screenshot-frame 300
        (the open left palm towards the eye; ...102 is Tarkov's bare hands, no --wear its default gloves, ...103 the SEVA's)
target\debug\trp.exe --wear 66f0a5740000000000000101 --play akm_check_0 --at 2.6 --fps off --tap 30:f1:4 --screenshot local\shots\b.png --screenshot-frame 300
        (the left hand from the thumb's side, holding the magazine, its wrist bent)
set EFT_OUTFIT_ARMS=both     (bash: EFT_OUTFIT_ARMS=both target/debug/trp.exe ...)
        (the outfit's arms and the install's own in one picture; add --arms assets/content/hands/hands_bear_telnik/hands_bear_telnik.skin.bundle for the bare ones)
target\debug\trp.exe --wear 66f0a5740000000000000101 --custom-hands off ...
        (the options' switch off from the start: the install's default arms with the suit on)
target\debug\trp.exe --wear 66f0a5740000000000000101 --inventory --play akm_look --at 1.5 --fps off --drag 1490:78:1490:78 --drag 400:125:400:125 --drag 40:172:40:172 --tap 660:tab:6 --screenshot local\shots\c.png --screenshot-frame 800
        (the switch thrown by a click, the screen shut, the hands looked at; frame 640 without the tab key shows the switch itself)
powershell -File local\crop_shot_s43.ps1 local\shots\a.png local\shots\a_crop.png 380 520 420 380 2
        (the left hand of the look at 1.5 s, twice as large; 560 660 480 240 for the hand at the magazine)
cargo test -p trp-data --test install outfit -- --nocapture
        (the arms' size, the wrists, the palm from edge to edge beside the install's gloved and bare hands; every outfit's palm)
```

- The captures of this are in `local\shots\s45` (the user's own picture among them, `user.png`); `local\hands_explore_s45.txt` is the last table of the scratch test that measured the hands slice by slice and strip by strip (the test itself, `crates\trp-data\tests\hands_explore.rs`, was a session's and is gone: `palm_box` is what was kept of it).
- **Reading two captures of dark gloves at their own size misleads**: a glove against a dark sleeve reads narrower than it is. Cut the hand out and enlarge it, or draw both sets of arms at once.

## Added 2026-10-14 (the smooth animations' switch)

```
target\debug\trp.exe --smooth-animations off --fps off --tap 200:r:7 --log-frames local\shots\r_off.txt --screenshot local\shots\r_off.png --screenshot-frame 1000
target\debug\trp.exe --smooth-animations on  ... (the same, r_on)
        (a reload, every drawn frame logged; --tap 200:l:5 for the look all over)
awk -v PREFIX=RELOAD -f local/palm_jerk_s46.awk local/shots/r_off.txt
        (each palm's change of velocity a frame over the frames of states beginning so: mean, most, and the stutter, root mean square, middle and ninth tenth; PREFIX=LOOK, IDLE)
awk -v FROM=520 -v TO=560 -f local/palm_speed_s46.awk local/shots/r_off.txt
        (the first palm's speed and its change, frame by frame: the steps are seen by eye)
target\debug\trp.exe --inventory --fps off --drag 1490:78:1490:78 --drag 400:125:400:125 --drag 40:207:40:207 --tap 660:tab:6 --tap 760:r:7 --log-frames ...
        (the second switch thrown by a click, the screen shut, a reload logged)
bash local/regress.sh after47     (kept: after47, the second switch's; after46's in every figure)
```

- The captures and logs of this are in `local\shots\s46`.

## Added 2026-10-14, evening (the inventory's pictures; the invisible draw; the freeze)

```
cargo run -q -p trp-data --bin probe -- icon 59d6088586f774275f37482f       (an item's PreviewPivot: the turn, the shift, the bounds' scale, the object it is on)
cargo run -q -p trp-data --bin probe -- resources IconCamera              (resources.assets' game objects by a part of their name: components, layer, transforms, lights, behaviours)
cargo run -q -p trp-data --bin probe -- studio                            (the icon camera as the code reads it, and whether it is the hand reading: Studio::as_read)
cargo test -p trp-data --lib icon                                         (the finishing and the quarter turn)
cargo test -p trp-data --lib texture                                      (the BC5 re-packing: 4000 random blocks, a DXT5 picture)
rm -rf local/icon_cache                                                   (every picture drawn again on the next run)
target\debug\trp.exe --loot --bare --wheel 200:-12 --screenshot local\shots\x.png --screenshot-frame 500
        (the crate's pictures, scrolled; --inventory --equip for what is worn and carried)
```

- **What a run made ready, and when**: the log's lines `made ready ... put into the world in X ms` (a rig) and `the icon studio: ...`, `the inventory's pictures: N read from the cache, M drawn, K not made` (one each time what was asked is all made); `--log-pacing` shows a long frame if one is left.
- The captures of this are in `local\shots\s47` (the invisible draw and the freeze) and `local\shots\s48` (the pictures: `loot9.png`, `equip.png`).
- **The cache is the app's own**, `local\icon_cache\v1`; the folder's name changes when the drawing does, so an old cache is never read by a new drawing. The game's own cache (`Icon Cache` in its temporary folder) has no item in it on this machine.

## Added 2026-10-14, night (the other game's archives; the suits' pictures; the third switch)

```
cargo run -q -p trp-data --bin probe -- xdb "meshes\dynamics\outfit\s" get
        (every archive with its count of files and the names with the words; with get, each read, unpacked and checked, and how long it took)
cargo run -q -p trp-data --bin probe -- ogf dynamics\outfit\stalker_outfit.ogf
        (a model read loose or from the archives: says which archive)
cargo test -p trp-data --lib xray_archive                                      (the CRC, LZO1X by hand, LZHUF through a test encoder)
cargo test -p trp-data --test install archives_are_read_whole every_outfit -- --nocapture
target\debug\trp.exe --loot --bare --fps off --screenshot local\shots\s49\loot.png --screenshot-frame 500
target\debug\trp.exe --inventory --fps off --drag 1490:78:1490:78 --drag 400:125:400:125 --drag 40:242:40:242 --screenshot ... --screenshot-frame 500
        (the features part, the third switch thrown: RESTART REQUIRED bright)
target\debug\trp.exe --loot --bare --fps off --icons off --custom-hands off ...
target\debug\trp.exe --wear 66f0a5740000000000000101 --bare --view body --yaw 160 ...   (the Sunrise's rucksack)
```

- The captures are in `local\shots\s49`; the suits' pictures are `local\icon_cache\v1\66f0a57400000000000001*_190x190_<checksum>.png`.
- **A log written with PowerShell's `*>` is UTF-16**: `grep` from Git Bash finds nothing in it. Use `Select-String`.

## Added 2026-10-14, late night (the first start's questions; a fresh copy)

```
"C:\nowhere`n`"E:\spt`"`nE:\GAMMAEFT`n`n" | target\debug\trp.exe --setup --loot --bare --icons off --screenshot <dir>\shot.png --screenshot-frame 200
        (from a folder with no workspace above it, so that its own local\ is written: PowerShell)
cargo test -p trp-app setup
cargo test -p trp-data --lib install
git ls-files -z --cached --others --exclude-standard | xargs -0 cp --parents -t <dir>
        (from E:\stlkreft in Git Bash: exactly what would be published, to build where no local\ above it is found)
git log --all --format= --name-only --diff-filter=A | sort -u        (every file ever committed)
```

- A fresh copy must lie outside `E:\stlkreft`: inside it, `local\eft_dir.txt` and the rest are found by looking upward, and the copy would run on this machine's settings. Its build is 5.7 GB; remove it after.

## Added 2026-10-14, the small hours (the crates renamed: TRP)

```
perl <scratch>\rename_trp.pl <files>        (the swap: eft-data/sim/app, eft_data/sim, eftpc to trp-*, trp_*, trp; literal, bytes in and out)
bash local/regress.sh after48     (kept: after48, the rename's; after47's in every figure, the reports the same to the byte but for the clips' load time)
for n in 3 ... 16 v vl; do cmp local/r${n}_replay_after48.txt local/r${n}_replay_after47.txt; done
        (metricdiff.sh says nothing when nothing moved, and nothing when it found no pairs: cmp tells the two apart)
```

- The program is `target\debug\trp.exe` now; the old `eftpc.exe` may still lie in `target\debug` from before, and is stale.

## Added 2026-10-14, late (the quick reload, the switches, every weapon, pictures when first wanted)

```
bash local/regress.sh after49     (the quick reload; after48's to the byte)
bash local/regress.sh after50     (the switches; after48's to the byte)
bash local/regress.sh after51     (kept: the sights told to every animator, the single barrel, the rocket launcher;
                                   after48's in every figure, the M3's report but for the clips' load time)
powershell -File local\alttab2_test_s3f.ps1   (the app in its own process: Alt+Tab away and back, a click on the window,
                                   the pointer on the other screen, the inventory open; the hold and the pointer each time)
trp.exe --equip --tap 150:lmb:3 --tap 400:r:3 --tap 420:r:3 --log-frames ...   (a quick reload: R twice in 20 frames)
trp.exe --start rifles --tap 60:f:3 --tap 330:alt:60 --drag-at 340 --drag 1400:500:1400:500 --tap 520:tab:3 --tap 800:3:3
                                  (the crate of rifles opened, the M4A1 put on the back with Alt and a click, drawn)
trp.exe --feature movement=off --hold w --log-pacing ...    (any switch for a capture, which reads no options file)
trp.exe --weapon rshg --equip --tap 200:rmb:3 --tap 400:lmb:3  (the RShG-2: the sights up, then the shot)
```

- **`sed -i` in Git Bash writes a file back with LF line ends**, whatever it had: twice this session on a CRLF source (put right with PowerShell's `ReadAllText`/`WriteAllText`). Edit with the Edit tool or PowerShell.
- Scratch of this session in `local\`: `*_s3f.*` (the specs' sections as written, the edit scripts, logs, captures in `local\shots\*_s3f.png`).

## Added 2026-10-14, later still (every weapon run whole)

```
cargo test -p trp-sim --test every_weapon -- --nocapture   (every weapon of the crates: assembled, its skinned parts'
                                   bones, drawn, fired, reloaded both ways, put away; a line each, with its id; 16 s)
powershell -File local\every_weapon_app_s3g.ps1   (each of those as the app's weapon: fired at 150, R at 300, aimed at 900,
                                   captured at 1100; a line each in local\every_weapon_app_s3g.txt, the logs in
                                   local\every_app_s3g\, the captures in local\shots\every_s3g\; 11 s a weapon, 28 min;
                                   -Only <name>,<name> for some, added to the file)
trp.exe --start rifles --tap 60:f:3 --drag-at 340 --drag 1305:160:1305:160 (ten times) --drag 1150:505:1150:505
        --wheel 356:-120 --tap 498:alt:30 --tap 560:tab:3 --tap 700:3:3   (the crate of rifles scrolled to its end by the
                                   wheel, the pointer kept on it by drags onto themselves; the AS VAL put on with Alt and
                                   a click, its rig made ready away from the frame, drawn from the back)
```

- **The loop's `R` at frame 300 falls inside a slow weapon's shot** (the Rhinos' 30 rounds a minute, the bolt actions), where it is not heard: those are not reloaded in the app's run, and the test reloads them. It is a check that nothing stops, not of the reloads.
- **A log written with `*>` from Windows PowerShell is UTF-16 and wrapped at the console's width**: a line with a long name goes on below (read it with `iconv -f UTF-16LE` in Git Bash; match the start of a line, not its end). **A script's `param()` must be its first statement**: after any other it is taken for a command, and the parameter is never bound.
- Scratch of this session in `local\`: `*_s3g.*`.

## Added 2026-10-14, night (what goes with a thing, lit)

```
trp.exe --start rifles --tap 60:f:3 --drag-at 340 --drag 1410:500:1410:500 --screenshot local\shots\x.png --screenshot-frame 420
                                  (the pointer left on the M4A1 by a drag onto itself: what goes with it lit;
                                   1040:550 is a STANAG, 1358:415 a stack of PS; captures local\shots\lit_*_s3h.png)
trp.exe ... --drag 1040:550:760:650 --screenshot-frame 350   (a STANAG carried, half way: its weapons lit)
trp.exe ... --feature compatible_items_highlight=off ...      (the same with the switch off: the M4A1 lit white alone)
trp.exe --inventory --drag 1490:78:1490:78 --drag 405:116:405:116 --screenshot ... --screenshot-frame 400
                                  (the options' FEATURES part: the fifteen switches over the list)
```

- Scratch of this session in `local\`: `*_s3h.*`.

## Added 2026-10-14, night (every weapon made when wanted; a box that blinks)

```
cargo test -p trp-sim --test every_weapon the_empty_hands -- --nocapture   (the empty hands have every bone a weapon shares)
powershell -File local\four_lazy_s3j.ps1   (the AKM, the StG 44, the M3, the Five-seveN, each the crate's weapon at a bare
                                   start: Alt and a click, Tab, its slot's key, a shot; whether its rig was made when wanted)
trp.exe --tap 40:tab:3 --drag-at 100 --drag 170:455:170:600 --screenshot-frame 150   (the AKM in hand dragged from the sling
                                   to the back: it blinks while it is put away; a later frame, whole)
trp.exe ... the same, fifteen drags onto the empty pockets, then --drag 170:600:170:455 --screenshot-frame 406
                                  (dragged back to the sling, the slot last drawn from: it blinks until it is drawn)
trp.exe --bare --loot ...          (a run begun with nothing in hand, the airdrop crate open: only the empty hands' rig)
```

- Scratch of this session in `local\`: `*_s3j.*`.

## Added 2026-10-14, night (the freeze when a weapon is first taken)

```
powershell -File local\freeze_s3k.ps1 [-Tag x]   (the four from the crate at a bare start, each with --log-pacing: the
                                   longest frames after the curtain; logs local\freeze_<name>_<tag>.txt)
$env:TRP_RENDER_TIMING = '8'; trp.exe ...       (prints the renderer's frames and the main thread's longer than 8 ms,
                                   stage by stage; 20 ms when the variable is not a number; Remove-Item Env:\TRP_RENDER_TIMING after)
```

- `--screenshot` costs its own frame (20 ms, frames 901 to 905 in these runs), and an inventory move draws the screen afresh (some 14 ms, frame 134): neither is a weapon's loading.
- Scratch of this session in `local\`: `*_s3k*`.

## Added 2026-10-14, night (what a rig costs; a move's slow frame)

```
trp.exe --bare --measure-rigs <crate|all>   (every weapon of that crate made ready and weighed, none put into the world:
                                   a line each and the sum; then the run ends; RIFLES, PISTOLS, ... or all)
cargo test -p trp-data --test install a_weapon_of_one -- --nocapture   (each picture of a weapon of one's own, compressed:
                                   what comes back, channel by channel)
cargo test -p trp-sim --test install reweighing_is_timed -- --nocapture   (what reweighing costs)
$env:TRP_RENDER_TIMING = '5'; trp.exe --bare --loot --drag-at 120 --drag 1385:205:1385:205 --tap 116:alt:20 --log-pacing ...
                                  (the vest put on from the airdrop crate: the move's frames, with the systems that say so)
```

- In a `--log-pacing` file a frame's `dt` is the time up to it: the work of a slow `dt` is in the row before it, which its `update` shows.
- Scratch of this session in `local\`: `*_s3l*`.

## Added 2026-10-14, night (the character model; rigs ahead, let go of, shared)

```
cargo test -p trp-sim --test standing -- --nocapture   (the stance of the game's screens: where the hands, feet and head are)
cargo run -p trp-data --bin probe -- animator <bundle>    (the animator of a bundle's prefab and its controller)
trp.exe --inventory --equip --screenshot local\shots\x.png --screenshot-frame 300   (the character model in the kit)
trp.exe --loot --bare --weapon 5c488a752e221602b412af63 --add-weapon 5dcbd56fdbd3d91b3e5468d5 --drag-at 120
        --drag 1330:175:170:450 --drag 1120:175:1120:175 --screenshot-frame 6600 ...
                                  (the MDR 7.62 hung, the 5.56 made ahead by the mouse resting on it, its pictures
                                   shared, and let go of 30 s on: the log's lines from `trp::weapon`)
```

- Scratch of this session in `local\`: `*_s3m*`.

## Added 2026-10-14, night (DirectX 12's places for samplers; the stuck pictures)

```
$env:TRP_ICON_CACHE = '<a folder>'   (a run's own icon cache, an empty one to see pictures made; the kept one is left be)
$env:TRP_ICON_TRACE = '1'            (every 60 frames: the studio's queue and drawing, and each pipeline still being built)
$env:TRP_RENDER_TIMING = '5000'      (with it, a `samplers:` line whenever the places held move: materials, lined, interface pictures)
cargo run -p trp-data --bin probe -- pointers level44 "InventoryScreen/Items Panel/Stash Panel/Simple Panel"
trp.exe --bare --start rifles --tap 60:f:3 --drag-at 340 --drag 1145:175:1160:175 --drag 1250:520:1250:520 ... --wheel 820:-12
                                  (rifles taken up one after another with empty clicks between, then the crate scrolled:
                                   seven at 1145:175 1385:225 1225:345 1120:445 1410:500 1300:600 1170:710)
```

- Scratch of this session in `local\`: `*_s3n*`.

## Added 2026-10-14, night (the tables of weapons; the window; DXC)

```
bash local/dxc_compare_s3o.sh <scratch folder holding dxc\dxcompiler.dll and dxc\dxil.dll>
                                  (FXC against DXC, two runs each: the warm-up, the crate's pictures on an empty cache, a walk)
trp.exe --bare --start window --hold w+vault --log-pacing local\x.txt   (through the window in the vault's state)
trp.exe --bare --start rifles --tap 60:f:3 --tap 230:tab:3 --screenshot-frame 236   (the gun case opened, its lid up as the screen shuts)
trp.exe --bare --start rifles --tap 20:c:3 --tap 140:f:3   (crouched, the magazine box under the table opened)
cargo test -p trp-app the_window -- --nocapture   (the window's sill as the simulation's grid measures it)
```

- Scratch of this session in `local\`: `*_s3o*`.

## Added 2026-10-14, night (the L; pictures whole)

```
$env:TRP_ICON_CACHE = '<an empty folder>'; trp.exe --bare --start rifles --tap 60:f:3 --drag-at 250 --drag 1250:400:1250:400 --wheel 700:-6 --wheel 1100:-6 ...
                                  (the rifles' case's pictures made with a rig made ahead meanwhile: the log's
                                   "its model all on the graphics card N frames after ..." lines are those the old wait cut short)
trp.exe --bare --start machine-guns --view map:9   (the L from above)
trp.exe --bare --start clothing --tap 60:f:3 --tap 230:tab:3 --screenshot-frame 236   (the clothing crate's lid open)
```

- Scratch of this session in `local\`: `*_s3p*`.
