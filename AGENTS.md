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

- **Do not code until told:** character values in a menu, values taken from STALKER, a host's game
  feeding a system (2026-10-14: "After we have almost all features working good, we will attempt to
  merge games"), co-op. Researched: the end of `docs\findings.md`.
- **Every weapon selectable** (2026-10-14, late): all of the install's along the knee-high wall,
  labelled by class ("Separated like (Rifles) (Pistols) (Shotguns) (Snipers) (Custom)";
  `range::WEAPON_CRATES`, `weapon_crates` in `main.rs`): since that night **a table for each, the
  guns in a green gun case on it, their magazines in a black box under it, lids that open** (the
  game's `WeaponCrate` and `AmmoBox`), the last four in an arm: an L; **the run's own weapon in its
  class's case**; the airdrop crate is **CLOTHING**, with a lid. A window to vault, by the door.
  **Found in a container a weapon has no round in its chamber** and works its action when taken in
  hand; a run that begins armed has one chambered. **Every weapon's rig is made when first wanted,
  the run's own too** (2026-10-14, night: "because they should"); a box blinks as the game's does. A weapon of one's own: the StG 44 (2026-10-13),
  from the user's model in `local\ExperimentalWeapon` (never in the repository).
- **The switches of the options, each "restart required"** (a run holds those it began with):
  `CUSTOM FIRST PERSON HANDS`, `SMOOTH FIRST PERSON ANIMATIONS` (a departure in the drawing alone),
  `ICON GENERATION`, `COMPATIBLE ITEMS HIGHLIGHT` (2026-10-14, night: the orange; off, Tarkov's
  white alone), `CHARACTER MODEL` (the same night), and since 2026-10-14, late, **every system
  the list has as built** ("For now,
  when set to off, lets make it just stop"; `settings.md`, "Features"; `trp_sim::Systems`).
- **The quick reload is the game's**, on `R`'s double click (2026-10-14, late); **with a screen
  open the mouse is free to leave the window** ("so I can just go to other tabs").
- **Outfits from STALKER** (2026-10-13): suits in the crate, on the armour's slot, in place of the
  player's model, read from the user's own GAMMA folders, never written or copied; what one has not
  got is Tarkov's default; pictured from that game's model of the item.
- **The options are a tab of the inventory, at its far right** (2026-10-12): rebinds; graphics; the
  features; `QUIT` ends the run (`docs\spec\settings.md`). **Doors have the game's menu**, the wheel
  choosing. **The weapons' animations go by the skills.**
- **The inventory** (2026-10-11): Tarkov's tabs exactly, its slots where the game has them over
  **the character's own model, still, as the pre-raid screen stands it** (2026-10-14, night:
  "loaded from your current model"; off, a paper figure); the app starts bare with crates to loot.
  **No stash.** Still wanted: looting as an animation. **The inventory's pictures are made when the screen first shows an item** (2026-10-14:
  "Check how tarkov handles that"), as the game does. **What goes with the thing under the mouse is
  lit orange**, either way round (2026-10-14, night: "Something like what stalker does"): a
  departure of the screen's, the user's (`inventory.md`, "The box under the mouse").
- **STALKER** (2026-10-11): for now, Tarkov's motion and features on STALKER's own arms and weapons;
  the engine may be patched. **Complete the character controller first.**
- The recorder may be changed however the work needs; running it is the user's. The stance, speed
  and stamina display stays plain by default. A tool the user points to is judged by whether it
  speeds the work.
- **Published for others to build and change** (2026-10-14), **as TRP, the Tushonka Runtime
  Project**: they bring their own SPT (Single Player Tushonka) and, if any, STALKER; a first start
  asks where those are (`setup.rs`). **Two repositories kept**: work in `stlkreft` only; a release is
  copied to `TRP` (`docs\manual.md`, "Publishing"; `E:\TRP-public` is cloned again if it is gone).
- **The version is raised with every change** (2026-10-14): `TRP\Cargo.toml`, Semantic Versioning
  below 1.0 (`docs\manual.md`, "Versions"); say the new number when a piece of work lands.
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
  options tab, the mouse's rate, the slow lean, the walk key, the backpack's key, the looks at the
  weapon; the door's menu and the lock; the Five-seveN, the Model 700, the M3's fold; the skill's
  speeds; the inventory's layout; empty hands; the character as others see it (`F5`); the free look;
  the StG 44 on the SCAR-L's rig (`custom-weapons.md`); the five kinds of 2026-10-13
  (`more-weapons.md`) and the donors' catalogue; STALKER's suits as the body (`outfits.md`); the
  smooth hands; the inventory's pictures the game's way (`icons.md`).
- **2026-10-14, late to night** (`status.md` has each, newest first): the quick reload; eleven
  more switches; all 149 weapons, on tables by class; rigs and pictures made when first wanted, a
  weapon's ahead by the mouse, rigs let go of, pictures alike shared; no freeze on a first take or
  a move; the gear page's character model; DirectX 12's 2,048 sampler places kept to
  (`weapon::SamplerPlaces`); pictures in sight only, as the game's grid; DXC where found (the
  warm-up 3 to 4 times sooner); a window to vault; pictures read back only once on the card (the
  old guess cut 26 of 38 short). Seen in captures, not tried by hand.
- **The pointer and the window** (`cursor.rs`): after anything that touches the window or the keys'
  way in, run `local\window_keys_test.ps1`; `local\alttab2_test_s3f.ps1` tries Alt+Tab four ways.
- Version 0.9.1 (0.1.0 published); 305 tests pass; regression `after53` (`after48`'s in every
  figure). Recorder 0.6.2 has run; 0.7.0 is built (`\` switches the player to third person; the
  mouse's axes are on record). **Not started:** health, healing, hunger, skills as a system, sound,
  the STALKER host.

## Next steps

1. **The RPK-16's magazine** (`5beed0f50db834001c062b12`): assembled with nothing in `mod_magazine`
   (in hand, its logic's "magazine of 0", its picture); the kit's vest got three of its magazines,
   so the database names one. Read its stock preset (`Database::default_preset`, `armed` and
   `fed_from_magazines` in `main.rs`, `trp_data::weapon::assemble`) and check the other weapons.
2. **The health system** (the user, 2026-10-14, night: "we are ready to start the health system"):
   research the game first (`Player`'s health controller, body parts, effects, healing, hunger and
   thirst, fall damage, the inventory's Health tab), spec it in `docs\spec\health.md`, then build it
   in `trp-sim` with a feature switch. Ask the user the questions below as they come up.
3. **If the user has recorded** (traces newer than 2026-10-12 21:34 in `E:\spt\BepInEx\plugins\
   traces`; `docs\manual.md`, "By hand"): to `local\traces\recording17` or `byhand5`, checked whole.
   Third person needs the replay to take a trace's point of view from its header; the free look needs
   `byhand` to read the mouse's axes. A quick reload and the RShG-2 are worth recording.
4. **Memory** (`findings.md`, "what a rig costs"): rigs are let go of after 30 s uncarried and share
   pictures alike, not meshes; `--measure-rigs` weighs them, `TRP_RENDER_TIMING` finds a slow frame.
5. **Scripts for the weapons that have none** (`scenarios-57`, `-m700`, a fold for the M3; the five
   kinds; the quick reload), each with an aimed scenario, and one run with a weapon skill level.
6. **The rest of the weapon kinds** (`more-weapons.md`, "Not built"): a cylinder's quick reload, a
   revolver's fistful, `LOAD ONE` with no magazine in reach, rounds, cases, grenades and rockets
   drawn. **For the suit** (`outfits.md`, "Not built"): its visor's glass, that game's normal and
   gloss pictures.
7. **The bindings that wait for a system** (`settings.md`, "Not built"): the pistol's fast draw, a
   magazine chosen, blind fire, the left shoulder. **The rest of each weapon** (the specs' "Not
   built"). Then empty hands' rest, looting item by item, the systems to come.
8. **Loose ends** (`findings.md`): a door's aiming point and when it snaps; elite Strength; the
   rounds in a weapon as things of the inventory as they are fired; a figure typed into a bar; the
   mouse's rate measured; **the selector's layer, which follows no switch of fire mode**.

## Open questions for the user

1. **The RShG-2** fires from the sights only. By the game's code a press from the hip starts its
   firing and then waits for good (built so; the hands stay locked): so in your game?
2. **The pointer after Alt+Tab** was taken again every way tried here: how did you come back?
3. **The tables of weapons**: your five and SMGS, MACHINE GUNS, LAUNCHERS for the rest; three spare
   magazines or two stacks each; the box under the table reached crouched; lids to 80 and 55: right?
4. **The Model 700 aimed** looks over the receiver from 7.8 mm left, the install's own bone: is
   that what your game shows? A scope is optics: wanted next?
5. **Skill levels** are an option (`--weapon-skill`): read them from your SPT profile, or the tab?
6. **Recordings from here on** are a check only (`docs\manual.md`, the short list): still agreed?
7. **The quick reload** by hand: does the old magazine fall where your game drops it (0.35 m ahead)?
8. **The switches** by hand: does off do what you meant for each (movement off: frozen in place;
   the first-person body off: no arms either)?
9. **The StG 44**: the AKM's recoil and 7.62x39, the left hand on the handguard, its sling: right?
10. **The character as others see it** (`F5`): as other players look in your game? A second one next?
11. **The outfits' hands** (1.27, the wrist on Tarkov's), the SEVA's glove, the rucksack: right?
12. **The smooth animations**: right in play, on and off? The legs below the eyes by it too?
13. **The inventory's pictures**: as your game's? **The orange** of what goes together: strong
   enough? Its switch leaves Tarkov's white on the box pointed at: so meant? **The StG 44's
   surface picture** in BC1 keeps its metal a little less exactly (up to 9% of pixels 8 off): BC7?
14. **The character model**: its framing and light (the app's), still and empty-handed by your word;
   should it hold the weapon in hand and turn by a drag, as the game's does? 30 s before a rig goes?
15. **DXC is taken from your Windows SDK** (or beside `trp.exe`): for others who build TRP, ship the
   two DLLs with a release, or the build feature that links it in (it downloads while building)?

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
bash local/regress.sh <name>  then  bash local/metricdiff.sh <name> after51  (9 min; or statecheck.sh)
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
