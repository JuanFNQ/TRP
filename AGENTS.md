# TRP (Tushonka Runtime Project)

Escape from Tarkov's first-person character controller, rebuilt in Rust on Bevy as a reusable runtime;
named TRP by the user on 2026-10-14. STALKER GAMMA is its first intended host. The code is the Cargo
workspace `E:\stlkreft\TRP` (`EFTPCRuntime` until 2026-10-14): crates `trp-data`, `trp-sim`,
`trp-app` (the program `trp.exe`; `eft-*` and `eftpc.exe` before). The private repository is still
`stlkreft`; the public one, `TRP`, is a copy without its history (`docs\manual.md`, "Publishing").

Read next, in this order: `TRP\README.md` (what it is, the state in short, how to run), then
the spec for whatever you are about to touch (`TRP\docs\spec\`). Everything else is in the
table under "Where the rest is".

## Goal

1. A Rust runtime that reproduces Tarkov's first-person controller: movement (speed, inertia, stance,
   lean, sprint, stamina, jump, vault, prone) and presentation (camera, arms, weapon animation, sway,
   breathing, recoil, aiming).
2. It reads its data from the user's own Single Player Tushonka (SPT) install (`E:\spt`) at runtime.
   The repo holds code only.
3. It is host-agnostic: a host feeds it input and collision and gets poses back. Long term it is a
   template to lay over other games, STALKER first.

"Exact" means measured against the real game, not judged by feel.

## Rules

1. **Research before code.** Prefer, in order: documented format, published source, decompilation.
2. **The installs are inputs.** Never write to `E:\spt`, `E:\AnomalyEFT` or other people's mods in
   `E:\GAMMAEFT\mods`. Copying the recorder into `E:\spt\BepInEx\plugins` is the user's job.
3. **No game content in the repo.** No assets, extracted files, decompiler output or pasted game code.
   Scratch goes in `TRP\local\` (ignored); decompiled code stays in `local\decomp\src`.
4. **Spec, then implement.** Write behaviour read from the game into `docs\spec\` in your own words,
   then write original code from that. **Meant for GitHub** (the user, 2026-10-14): the README's
   "What was learned, and how" says how the knowledge came, and must stay true.
5. **Every constant has a source:** read from the install, or cite the spec. A guess is marked
   `ESTIMATE`, an unconfirmed rule `PROVISIONAL`, and both are listed in `docs\status.md`.
6. **Claims are measured.** "Matches Tarkov" needs a trace comparison. Say what failed, what was only
   read from code, and what was not run.
7. **Ask before** launching either game, installing a toolchain, changing Mod Organizer, initialising
   git or publishing. Commits are the user's. Running the project's own app, tests and replay is fine.
8. **Do not depart from Tarkov** (the user, 2026-10-09). Where the code or a recording says what the
   game does, do that, even where it looks worse. The only liberty is how results are drawn between
   and across frames, and that must leave the simulation's results unchanged.

## Decisions of the user in force

- **Do not code until told:** character values in a menu, values taken from STALKER, a weapon of a
  kind not yet built, **anything switched by the options' list of features** (a list to be talked
  over first), co-op. Researched: the end of `docs\findings.md`.
- **Weapons told:** the M3 Super 90 (its stock folds), the FN Five-seveN, the Remington Model 700
  (2026-10-12); **one of each kind those are not**, "to run around and test" (2026-10-13: the SV-98,
  the SKS, the Rhino, the MP-43, the M32A1), with a catalogue of donors and rigs built when needed.
  **A weapon added goes into the airdrop crate**, larger inside than the install's, with a scroll bar
  (`WEAPONS` in `trp-app\src\main.rs`). **Found in a container it has no round in its chamber** and
  works its action when taken in hand; a run that begins armed has one chambered.
- **A weapon of one's own** (2026-10-13): the Sturmgewehr 44, from the user's model in
  `local\ExperimentalWeapon` (never in the repository), on the best of Tarkov's animations.
- **Three switches of the options, each "restart required"** (2026-10-14, night: a run holds those
  it began with): `CUSTOM FIRST PERSON HANDS` (2026-10-13; off, the hands do not change with what
  is worn), `SMOOTH FIRST PERSON ANIMATIONS` (a departure in the drawing alone), `ICON GENERATION`.
- **Outfits from STALKER** (2026-10-13): suits in the crate, on the armour's slot, in place of the
  player's model, read from the user's own GAMMA folders, never written or copied. **What one has
  not got is Tarkov's default body and gloves**; for its own eyes it is cut at Tarkov's waist; its
  hands are judged beside Tarkov's bare ones. **Pictured from that game's model of the item**
  (2026-10-14, night), with Tarkov's icon setup.
- **The options are a tab of the inventory, at its far right** (2026-10-12): rebinds; graphics;
  features to switch, a list only but for the three; as the game's own tab, `QUIT` ends the run, the
  app holds the mouse (`docs\spec\settings.md`). **Doors have the game's menu**, the wheel choosing.
  **The weapons' animations go by the skills.**
- **The inventory** (2026-10-11): Tarkov's tabs exactly (Overall, Map, Tasks, Prestige may stay off),
  its slots where the game has them over a paper figure; the app starts bare with a crate to loot.
  **No stash.** Still wanted: looting as an animation.
- **STALKER** (2026-10-11): for now, Tarkov's motion and features on STALKER's own arms and weapons;
  the engine may be patched. **Complete the character controller first.** Keep in mind, do not code:
  every system a host can switch on or off and feed from its own game (`docs\history.md`, "The systems").
- The recorder may be changed however the work needs; running it is the user's. The stance, speed
  and stamina display stays plain by default. A tool the user points to is judged by whether it
  speeds the work.
- **Published for others to build and change** (2026-10-14), **as TRP, the Tushonka Runtime
  Project**: they bring their own SPT, which is Single Player Tushonka, and, if any, STALKER; a
  first start asks in the console where those are (`setup.rs`). Making it public is the user's.
- **Keep this file and the README near 200 lines** (2026-10-13), detail in the documents below.

## Where things stand (2026-10-14)

- **Built and compared with the game, frame for frame** (sixteen scripted recordings and 41 by hand,
  kept as a check, not what the work is built from): the data pipeline and rig; the body's animator;
  movement; weapon motion, recoil, breathing; firing and reloading by the weapon's own animator; the
  arms' solver; stamina; the camera's collider; the ground pushing the weapon back; vaulting and
  climbing (three of four traces); the equipment's places and weights; the left hand's gestures;
  doors, containers, trunks and a kick; the M3 Super 90; the inventory's clicks and keys; the eye on
  the iron sights' line; the keys as the game's input manager hears them (a tapped `R`, 0.312 s).
- **From the code and the install, compared with nothing** (each spec says so at its end): the
  options tab, the mouse's rate (0.125 degrees a count), the slow lean, the walk key, the backpack's
  key, the looks at the weapon; the door's menu and the lock; the Five-seveN, the Model 700 (aimed,
  its eye is 7.8 mm left of the bore: question 3), the M3's fold; the skill's speeds; the inventory
  screen's layout; empty hands; a first weapon put on with hands never armed; the character as others
  see it (`F5`, `third-person.md`); the head's free look (the middle button, `free-look.md`).
- **The StG 44, which is not the game's** (`custom-weapons.md`, `custom\stg44.json`): on the FN
  SCAR-L's rig and animator with two edits to its pose; set beside its donor frame for frame; its
  round, recoil and cells `PROVISIONAL` or `ESTIMATE`.
- **Every weapon counted** (`probe weapons`): 128 of the install's 148 run on the four logics built
  first; **one of each of the other five kinds** (`more-weapons.md`: the SV-98, the SKS, the Rhino
  50DS, the MP-43e-1C, the M32A1), drawn, fired and reloaded in captures, neither recorded nor played
  by hand; **the donors' catalogue** (`donor --catalogue`, `--rank`); rigs made ready off the frame.
- **Outfits of STALKER as the character's body** (`outfits.md`, `xray.rs`, `outfit.rs`): the Sunrise
  and the SEVA from GAMMA's loose files (`local\gamma_dir.txt`) and **Anomaly's packed archives**
  (`xray_archive.rs`, all 68 read; the Sunrise's rucksack), on Tarkov's skeleton from outside and
  in the shadow; in first person cut at Tarkov's waist, that game's arms on Tarkov's finger joints and
  wrist at the joints' size (1.27): palms 1.09 and 1.18 times Tarkov's glove's. The Telnik, Tarkov's
  own, to set them beside; `CUSTOM FIRST PERSON HANDS` off keeps Tarkov's arms; what an outfit lacks
  is Tarkov's default. Seen in captures only.
- **2026-10-14**: the hands' animations drawn smooth or not (`firing.md`; the simulation the same, a
  lockstep test); a rig taken from the frame after it is built, its pictures re-packed as BC5 (the
  invisible first draw, the freeze: `findings.md`); **the inventory's pictures as the game makes
  them** (`icons.md`): each item's model in its prefab's pose under a copy of the game's icon camera,
  drawn once, kept in `local\icon_cache\v1`; a STALKER suit from that game's model lying on the
  ground, looked down on (the turn is this work's). Seen in captures, not compared with the game's.
- **The pointer and the window** (`cursor.rs`): after anything that touches the window or the keys'
  way in, run `local\window_keys_test.ps1`. **Not run:** fullscreen, the pointer after Alt+Tab.
- 282 tests pass; regression `after48`. Recorder 0.6.2 has run; 0.7.0 is built (`\` switches the
  player to third person; the mouse's axes are on record). **Not started:** health, healing, hunger,
  skills as a system, sound, the STALKER host.

## Next steps

1. **Ask the user** the questions below: 1 and 2 first; 8 to 13 once what they ask of has been
   tried by hand (all of it in the crate or on the options tab).
2. **If the user has recorded** (traces newer than 2026-10-12 21:34 in `E:\spt\BepInEx\plugins\
   traces`; `docs\manual.md`, "By hand"): to `local\traces\recording17` or `byhand5`, checked whole.
   Third person (item 11) needs the replay to take a trace's point of view from its header; the free
   look (item 12) needs `byhand` to read the mouse's axes.
3. **Scripts for the weapons that have none** (`scenarios-57`, `-m700`, a fold for the M3: the
   recorder needs a fold press; then the five kinds), each with an aimed scenario, and one run with
   a weapon skill level. For the keys, by hand: item 10, a tapped sprint key, `C` with the wheel.
4. **The rest of the weapon kinds** (`more-weapons.md`, "Not built"): one barrel reloaded alone and
   the rocket launcher's firing, which would make every weapon of the install selectable; the rounds
   and cases drawn; a revolver's fistful and quick reload; a kind for a revolver in `donor --rank`.
   Then the catalogue put to use: a second weapon of one's own, or a model from STALKER.
   **For the suit** (`outfits.md`, "Not built"): its visor's glass seen through, that game's normal
   and gloss pictures; Anomaly's archives are read now, so any of that game's content can be had.
5. **The bindings that wait for a system** (`settings.md`, "Not built"): the pistol's fast draw, a
   magazine as a thing, blind fire, the left shoulder. Then what the user settles of the features.
6. **The rest of each weapon** (the specs' "Not built"): the body's animator's two speeds, the M3
   in the inventory screen, the key in the hand. Then empty hands' rest, looting, the systems to come.
7. **Loose ends** (`findings.md`): a door's aiming point and when it snaps; the weapon line's
   length at a reload's end in the app; elite Strength; the rounds in a weapon as things of the
   inventory; a pack dragged off its slot; a figure typed into a bar; the mouse's rate measured;
   **the selector's layer, which follows no switch of fire mode in the simulation, the AKM's either**.

## Open questions for the user

1. **The reload's wait is your game's** (0.312 s: the fast reload's double click on `R`): keep it,
   or move that double click off `R`? By hand: do the mouse and the free look turn as far as in your
   game for the same hand travel, do keys with Alt held work, does the pointer lock after Alt+Tab?
2. **The features' list** (the tab's third part): which are to be switches, and does a switch
   turn a system off, or hand it to the host's own game (weight, hunger, radiation)?
3. **The Model 700 aimed** looks over the receiver from 7.8 mm left, the install's own bone: is
   that what your game shows (a screenshot would settle it)? A scope is optics: wanted next?
4. **Skill levels** are an option (`--weapon-skill`): read them from your SPT profile, or the tab?
5. A first weapon taken by hands empty all raid waits for its key (the code's way): so in your game?
6. The crate's weapons full, none chambered, ten by thirty cells, the other containers empty: right?
7. **Recordings from here on** are a check only (`docs\manual.md`, the short list): still agreed?
8. **The StG 44**: it kicks and handles as the stock AKM and fires the install's 7.62x39: right, or
   other figures? The left hand on the handguard (as built) or on the magazine well? Its sling? And
   may a trigger reach every layer of an animator (the selector's, step 7)? That touches the simulation.
9. **The character as others see it** (`F5`): does it look as other players do in your game? Is a
   second character on the range wanted next, or is that co-op's and to wait?
10. **The five new weapons:** the revolver and the launcher are found full, the double barrel empty:
   right? Next: rounds and cases drawn, the one-barrel weapons, or the grenade in flight? Record them?
11. **The outfits' hands** (1.27, the wrist on Tarkov's): right, or set aside by the switch? The SEVA's
   glove too broad (`arms_size`)? Their figures the PACA's, or the other game's? The rucksack right?
12. **The smooth animations**: right in play, on and off? The legs below the eyes by it too? A
   recording of `L` ("By hand", item 14) says whether the game's own look stutters.
13. **The inventory's pictures** (the game's way): do they look as your game's (a screenshot beside
   the app's would say)? The suits looked down on, head up: as you meant? Kept behind the curtain
   (a first run waits ~8 s once; drawn in play, 20 to 90 ms stalls), or drawn in play after all?

## Working here

Run from `E:\stlkreft\TRP`. The install path comes from `EFT_DIR` or `local\eft_dir.txt`.

```
cargo build --workspace
cargo test --workspace
cargo run -p trp-app                                   (the app; docs\manual.md has keys and options)
cargo run -p trp-app -- --weapon rhino_50ds --tap 200:lmb:6 --screenshot local\shots\x.png --screenshot-frame 700
cargo run -p trp-sim --bin replay -- --traces local\traces\recording13 06
cargo run -p trp-sim --bin byhand -- local\traces\byhand3          (a recording made by hand)
cargo run -p trp-data --bin probe -- states 61a4c8884f95bc3b2c5dc96f reload   (also: weapons, item)
cargo run -p trp-sim --bin donor -- --rank custom\stg44.json       (--catalogue makes its file)
bash local/regress.sh <name>  then  bash local/metricdiff.sh <name> after48  (9 min; or statecheck.sh)
```

- **After any change to the simulation, run the regression** and compare with the last kept run.
- Look at a capture before judging a rendering change; pick a late frame; build the app first
  (`cargo test -p trp-app` does not rebuild `trp.exe`). If cargo cannot replace the exe, the user
  has the app open: wait, never close it, start no second one (`cargo check` meanwhile).
- In PowerShell 5.1 do not use `2>&1` on cargo: redirect with `*>` to a file under `local\`. Build the
  recorder from PowerShell; the .NET SDK is at `C:\Users\Owner\AppData\Local\Microsoft\dotnet\dotnet.exe`.
- Long scripts, test code and text to append go into a file with the Write tool: a long
  here-document fails, `perl -pe` mangles Rust and `sed -i` turns a file's line ends;
  `local\apply_edits.pl` swaps exact text (`workbench.md`; run it from Git Bash).
- A new recording overwrites `E:\spt\BepInEx\plugins\traces`: copy it to `local\traces\recordingN`
  first, check it is whole (`local\r6_health.ps1`), give it a `ground.txt`.
- The decompiled game is `local\decomp\src` (one file a type). Before taking a rule from a class, check
  that something makes or calls it, and which player gets it: several classes there are never used.
- Look before writing a file in `local\` (it holds working tools); give session scratch a suffix.
  Bevy is pinned to 0.16. No Python or Node on this machine. A session that was cut off left a
  transcript; `workbench.md` says how to read it back.

## Where the rest is

`crates\trp-data` reads the install (`bin\probe`), `crates\trp-sim` is the controller with no engine
types (`bin\replay`, `bin\byhand`, `bin\donor`), `crates\trp-app` the Bevy app, `hosts\spt-oracle` the
C# recorder, `custom\` the figures of weapons of one's own; `local\` is ignored scratch (traces,
decompiled code, tools, captures, the donors' catalogue).

| For | Read (under `TRP\`) |
|---|---|
| What it is, the state in short, how to run, the layout | `README.md` |
| The state in full: what works, what is missing, **every placeholder** | `docs\status.md` |
| Every key, every option, every probe command, how to record and compare, the "By hand" list | `docs\manual.md` |
| How the game behaves, and what each part was compared with | `docs\spec\*.md` |
| The kinds of weapon and the five added; a weapon not the game's, the donors' catalogue | `docs\spec\more-weapons.md`, `custom-weapons.md` |
| A character seen from outside; the head's free look | `docs\spec\third-person.md`, `free-look.md` |
| A body from STALKER worn as an outfit: its model's format, the fit to Tarkov's skeleton | `docs\spec\outfits.md` |
| What the install, the code and the recordings have shown; traps; what was researched | `docs\findings.md` |
| The kept recordings one by one, the scratch scripts, the tools | `docs\workbench.md` |
| The log day by day, every decision of the user, each recorder version | `docs\history.md` |

## Keeping this file current

Update "Where things stand", "Next steps" and the questions when a step lands, and keep this file and
the README near 200 lines: the state in full goes into `docs\status.md`, a key, an option or a command
into `docs\manual.md`, a finding into `docs\findings.md`, a script or a recording into
`docs\workbench.md`, what was done and decided into `docs\history.md`, behaviour into its spec.
