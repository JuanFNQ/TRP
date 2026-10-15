# TRP (Tushonka Runtime Project)

A Rust rewrite, on Bevy, of the first-person character controller of the game Single Player Tushonka (SPT) runs on. It reads everything it shows from your own SPT install at startup. This repository contains code only: no models, textures, animations, sounds or game code. (One thing it can show is not the install's: a weapon of one's own, whose model lies outside the repository in `local\`.)

This is an unofficial fan project for study. It is not affiliated with or endorsed by Battlestate Games or GSC Game World. You need your own copies of the games to run it: see "What was learned, and how" at the end.

**This file is the short form.** The state in full, what is missing and every placeholder: `docs/status.md`. Every key, every option, every tool and how to record and compare: `docs/manual.md`. How the game behaves: `docs/spec/`.

## Status

The first-person controller is built and, wherever a recording can show it, compared with traces recorded from the running game: sixteen scripted recordings and forty-one made by hand (2026-10-07 to 2026-10-12). Since then the work is written from the game's code and the install, and a recording is a check: the last thirty-five changed nothing. **"Compared with" below is what "matches" means here; "nothing" means from the code and the install alone.**

| What | Spec (`docs/spec/`) | Compared with the running game |
|---|---|---|
| Reading the install: bundles, meshes, clips, animators, the database, the settings, the screens | | every test runs on it |
| The body's animator (22 layers), the body drawn, worn items | `body-animator.md` | its state on every frame of 60 scenarios (70,700 frames); the legs bone for bone, 0.2 mm |
| Movement, the character's own collision | `movement.md` | the movement state on every frame; distances within 4.4 mm; the eye within a millimetre or two |
| Vaulting and climbing | `vaulting.md` | three of four traces: vaults within 6 mm, the climb within 3 cm |
| Stamina | `stamina.md` | to four decimal places, the legs run dry included |
| Weapon motion, breathing, recoil, the camera's box, the turn-away | `weapon-handling.md` | 179 shots: the same least and most, means within 1.5% |
| Firing and reloading by the weapon's own animator (the AKM) | `firing.md` | the animator's state and time on 461,415 frames; the hand markers within 0.05 mm |
| The arms' solver | `hands-on-weapon.md` | 0.02 degrees and 0.2 mm, given the game's goals |
| The Benelli M3 Super 90 | `shotgun.md` | nineteen scenarios, every frame; the recording changed nothing |
| The inventory, looting, its clicks and keys | `inventory.md` | places and weights to the gram; sixteen recorded moves of sixteen |
| The inventory's pictures: each item's own model drawn once by a copy of the game's icon camera, kept in `local/icon_cache` (2026-10-14) | `icons.md` | nothing; seen in captures |
| Doors, containers, a trunk, a kick, the left hand's gestures | `interactions.md` | twenty-five recordings by hand: angles to 0.000 degrees, gestures within a frame |
| The keys as the game's input manager hears them, the mouse | `settings.md` | five bindings of five on the game's own frame; the rest and the mouse's rate: nothing |
| The options tab (rebinds, graphics, a list of features with three switches, each taking effect at the next start: the outfits' own hands in first person; every animation of the hands drawn smooth or as the engine has it; the inventory's pictures) | `settings.md` | nothing |
| The FN Five-seveN; the Remington Model 700; the M3's folding stock; the skill's speeds | `pistol.md`, `bolt-action.md`, `shotgun.md`, `firing.md` | nothing (the eye on the sights' line: measured for the M3 and the AKM) |
| **The SV-98, the SKS, the Rhino 50DS, the MP-43e-1C, the Milkor M32A1** (2026-10-13) | `more-weapons.md` | nothing |
| A weapon of one's own: the StG 44 on the FN SCAR-L's animations | `custom-weapons.md` | its donor, frame for frame; no game has it |
| The catalogue of donors: every weapon measured as a source of animations | `custom-weapons.md` | the choice made by hand (the same first four) |
| **Outfits from another game**: S.T.A.L.K.E.R.'s Sunrise and SEVA suits as the character's body, their own arms in first person (2026-10-13) | `outfits.md` | nothing (neither game shows it); their hands beside Tarkov's bare and gloved ones in captures |
| The character as others see it (`F5`) | `third-person.md` | nothing |
| The head's free look (middle mouse) | `free-look.md` | nothing |
| Empty hands | `inventory.md` | appearing: by hand; the rest: nothing |

305 tests pass, one of which runs every weapon of the crates through its logic. The regression over the sixteen kept recordings is run after every change to the simulation; the last (`after51`, 2026-10-14, after the quick reload, the switches and the last kinds of weapon) is the same in every figure as the one before them (`after48`).

**Not built** (the full list: `docs/status.md`, "Missing"): health, healing, hunger and thirst as systems, skills as a system; sound, muzzle effects, shell ejection, the rounds and magazines as things drawn; malfunctions, ballistics and dispersion, optics, mounting, blind fire, the left shoulder; grenades and the knife; a weapon with one barrel reloaded alone, the rocket launcher; slopes, sliding and falls' harm; searching item by item, using food or medicine; a second character; Tarkov's own shading (materials are an approximation); the STALKER host.

**Placeholders.** Every guess in the code is marked `ESTIMATE` and every unconfirmed rule `PROVISIONAL`; all of them are listed in `docs/status.md`, "Placeholders". The ones that show most: the mouse's 0.125 degrees a count (two figures of the install and the engine's halving, never measured); the StG 44's round, recoil and size; the test range, which is the app's own; the crates, which are larger than the install's, and the crates of weapons, which are the app's; a weapon found in one has a full magazine and nothing chambered; where a magazine let fall lies.

## Requirements

- Windows, Rust (stable, MSVC toolchain) with the Visual Studio C++ build tools.
- Single Player Tushonka (SPT) 4.1, installed over your own copy of the game (its client 0.16.9). SPT and the mods for it are on its Forge (`sp-tushonka.com`). The item database is read from SPT's files, so the game alone is not enough.
- A GPU that supports BC texture compression (any desktop GPU).
- Optional, for the suits from S.T.A.L.K.E.R.: Anomaly, alone or under GAMMA. Both are read, never written; without them the suits are items with nothing drawn for them.

## Running

```
git clone https://github.com/JuanFNQ/TRP.git
cd TRP\TRP
cargo run -p trp-app
```

**On the first start the console asks where your games are**: your Single Player Tushonka (SPT) folder (the one with `SPT_Runtime` and the game's `EscapeFromTarkov.exe`), and, if you have them, GAMMA's Mod Organizer folder and Anomaly's own folder (offered from Mod Organizer's settings; Enter skips either). Each answer is checked, and kept as a one-line file in `local/` (`eft_dir.txt`, `gamma_dir.txt`, `anomaly_dir.txt`). `cargo run -p trp-app -- --setup` asks again; the files can also be written by hand, or `EFT_DIR`, `GAMMA_DIR` and `ANOMALY_DIR` set instead. The first line printed is the version (`--version` prints it alone): Semantic Versioning below 1.0, raised with every change (`docs/manual.md`, "Versions").

The first build compiles Bevy and takes a few minutes. A run begins with a curtain of a second or two while the renderer builds what it draws with. An item's picture for the inventory is drawn the first time the screen shows the item, as the game does, and kept in `local/icon_cache` (delete that folder to have them drawn again); a weapon is made ready the first time it is hung on a slot. The character starts bare on a test range; **the crate straight ahead holds a rifle, a vest and a backpack, and the eight crates to its right along the low wall hold every weapon of the install**, labelled by class (look at one and press `F`; Alt and a click puts a thing on; `Tab` shuts the screen; `2`, `3` or `1` takes the weapon in hand).

**The keys are the install's own bindings** (`Control.ini`), each pressed the way the install has it; the guide at the top left of the display reads them off as they stand (`F1` hides it), and the inventory screen's last tab, `OPTIONS`, changes them. In the install this was written with:

| Key | Action |
|---|---|
| `W A S D`, mouse | Move, look |
| `Left Shift` | Sprint (a press on, a press off) |
| `Space` | Jump; held with `W`, vault or climb |
| `C`, `X` | Crouch or stand; lie down or get up |
| `C` + wheel, wheel | Stance; walking speed |
| `Caps Lock` | Walk slowly, and back |
| `Q` / `E` | Lean (hold); with `Left Alt`, step aside |
| `Left Alt` + `A` / `D` | Lean slowly; it stays |
| Middle mouse (hold) | Look around with the head |
| `Z` twice | Drop the backpack |
| Left mouse, right mouse | Fire; aim (a press on, a press off) |
| `Left Alt` | Hold breath, aiming |
| `R` | Reload from the fullest magazine in the pockets or the vest (0.3 s after the key: the install has a double click on it) |
| `R` twice | Quick reload: the old magazine is let fall |
| `Left Alt` + `T`, `Left Shift` + `T` | Check the magazine; look into the chamber |
| `B`, `B` twice, `Left Alt` + `B` | Next fire mode; the fullest; look at the selector |
| `L`, `Left Alt` + `L` | Look the weapon over; fold the stock |
| `2` / `3` / `1` | The weapon on the sling, on the back, in the holster |
| `F`, wheel with a menu up | Do the menu's chosen line for what is looked at; choose a line |
| `Tab` | The inventory screen (Ctrl and a click sends a thing across, Alt and a click puts it on, a double click opens it, `Delete` puts it down, `4` to `0` bind it) |
| `Escape` | Shut the window on top, or the screen |
| `F1`, `F3` | The guide to the keys; the frame-rate readout |
| `F4`, `F5` | A free camera; play while watched from outside |

**Options most used** (all of them: `docs/manual.md`, "Running"):

```
cargo run -p trp-app -- --weapon rhino_50ds          begin with a weapon in hand, by its id or a part of its name
cargo run -p trp-app -- --add-weapon m4a1            lay another copy of a weapon in the airdrop crate
cargo run -p trp-app -- --equip                      begin with the crate's rifle, vest and backpack on
cargo run -p trp-app -- --view watch                 watched from outside (also fp, side, body, map)
cargo run -p trp-app -- --weapon-skill 50 --mastering 2 --weight 30 --fov 75
cargo run -p trp-app -- --tap 200:lmb:6 --tap 420:r:6 --screenshot local\shots\x.png --screenshot-frame 700
```

The last is a capture: keys pressed on given frames through the bindings (`--tap`, `--drag`, `--mouse`, `--wheel`, `--hold`), one frame saved, and the run ends. A capture takes the keys and the mouse while it runs.

## The weapons

Every weapon of the install with a stock preset (148) and the one of one's own lie in the gun cases on the tables of weapons, eight of them by class, with what loads them in a box under each (`docs/spec/inventory.md`). Which logic a weapon runs on goes by its template (`docs/spec/more-weapons.md`); every kind the install has runs, and `--weapon` begins with any of them in hand.

| Weapon | Item | Kind | Compared with |
|---|---|---|---|
| Kalashnikov AKM | `59d6088586f774275f37482f` | a magazine that comes out; single shots and automatic | fourteen recordings |
| Benelli M3 Super 90 | `6259b864ebedf17603599e88` | a tube filled a shell at a time; pumped or self-loading; a folding stock | the sixteenth recording |
| FN Five-seveN | `5d3eb3b0a4b93615055e84d2` | a pistol; the slide held back | nothing |
| Remington Model 700 | `5bfea6e90db834001b7347f3` | a bolt action, its magazine filled with the bolt open | nothing |
| Izhmash SV-98 | `55801eed4bdc2d89578b4588` | a bolt action whose magazine comes out | nothing |
| TOZ SKS | `574d967124597745970e7c94` | a self-loader filled through its action | nothing |
| Chiappa Rhino 50DS | `61a4c8884f95bc3b2c5dc96f` | a revolver, single and double action | nothing |
| MP-43e-1C | `5580223e4bdc2d1c128b457f` | two barrels loaded one by one, fired singly or together | nothing |
| Milkor M32A1 | `6275303a9f372d6ea97f9ec7` | a launcher with a drum a spring turns (its grenade does not fly) | nothing |
| Sturmgewehr 44 | `66f0a5740000000000000044` | not the game's: a model of one's own on the FN SCAR-L's animations (`custom/stg44.json`) | its donor |

A weapon found in a crate has a full magazine, tube or cylinder and no round chambered, and works its action when first drawn; a weapon with barrels is found empty, the rocket launcher with its rocket. Beside each lie three spare magazines, or two stacks of its rounds: the reloads take them from the pockets and the vest worn (`R`).

**Three outfits** lie in the crate too (`docs/spec/outfits.md`); Alt and a click puts one on the body armour's slot. **The Sunrise suit and the SEVA suit** are S.T.A.L.K.E.R.'s, read from your GAMMA folder: each is then the character's body **with its own build** (broader than Tarkov's, as tall), on a rig of its own that follows Tarkov's animation, the hands on the weapon and the feet on the ground as Tarkov's are. It is drawn from outside (`F5`) and as the shadow; in first person it is cut at the waist as Tarkov's own body is, and **the arms are that game's own first-person arms for the suit**, every finger's joints and the wrist on Tarkov's own and the hand's skeleton the size of Tarkov's (1.27 times the model's file; a line `"arms_size"` in the outfit's file says another), through every animation of Tarkov's weapons. **`CUSTOM FIRST PERSON HANDS`, at the top of the options tab's `FEATURES` part, switches that off**: the hands are then Tarkov's default arms whatever is worn. **The Telnik** is Tarkov's own: a top with bare hands, to set the other two beside. **What an outfit has not got is Tarkov's default** body or arms. Not drawn: the Sunrise's rucksack (its picture is only in Anomaly's packed archives, which are not read yet). `--wear 66f0a5740000000000000101` starts in the Sunrise (`...102` the Telnik, `...103` the SEVA).

## Looking into the install, and comparing with the game

```
cargo run -p trp-data --bin probe -- weapon 59d6088586f774275f37482f          a weapon as assembled
cargo run -p trp-data --bin probe -- states 61a4c8884f95bc3b2c5dc96f reload   its animator's states, marks, conditions
cargo run -p trp-data --bin probe -- weapons                                  every weapon: class, states, clips
cargo run -p trp-data --bin probe -- item backpack                            an item of the database
cargo run -p trp-data --bin probe -- ogf actors\stalker_neutral\stalker_neutral_2.ogf   a model of S.T.A.L.K.E.R.'s: meshes, pictures, bones
cargo run -p trp-data --bin probe -- xdb dynamics\outfit get     that game's packed archives: the files with a word, read and checked
cargo run -p trp-sim --bin donor -- --catalogue                               measure every weapon as a donor (a minute)
cargo run -p trp-sim --bin donor -- --rank custom\stg44.json                  the donors nearest to a weapon of one's own
cargo run -p trp-sim --bin replay -- --traces local\traces\recording13 06     a recording replayed, gap by value
cargo run -p trp-sim --bin byhand -- local\traces\byhand3                     a recording made by hand, read
```

`hosts/spt-oracle` is a BepInEx plugin for Single Player Tushonka (SPT) that plays scripted input in a raid, or records what is done by hand, and writes what the game's own controller does frame by frame; `replay` plays the same scripts through the simulation and reports the gap for each value. Building it, the one step that puts a file into the game's folder (yours to do), every script set, the recorder's versions and **the short list of recordings still worth making** are in `docs/manual.md`, "Comparing with the game".

## Tests

```
cargo test --workspace
cargo test -p trp-sim -- --nocapture
```

The unit tests need nothing. The tests in each crate's `tests/` folder run against the configured install and pass with a note when there is none; with `--nocapture` they print what they measured. 305 pass.

## Layout

```
crates/trp-data   readers for the install: Unity bundles, the SPT database, weapon assembly, animator
                  controllers in full, the body's controller and skeleton, the settings object, the
                  inventory's items, the game's screens; a reader of binary FBX for a weapon of one's
                  own; a reader of S.T.A.L.K.E.R.'s models (xray.rs) and the fit of one to the
                  install's skeleton (outfit.rs); bin/probe looks into all of it
crates/trp-sim    the controller as a plain simulation with no engine in it: the animators, the body,
                  movement, stamina, vaulting, collision, the weapon's logic as the game's operations on
                  its animator (firearm.rs, firearm/chambers.rs), empty hands, the inventory, the things
                  the hands work, weapon motion, recoil, the arms' solver, the keys as the game's input
                  manager hears them; bin/replay, bin/byhand, bin/donor
crates/trp-app    the Bevy application: the range, the rigs, the inventory screen, the options tab
hosts/spt-oracle  the recording plugin for SPT (C#) and its scenarios
custom/           the weapons that are not the install's, one file of figures each (models in
                  local/), and under outfits/ the outfits (models in the other game's own folders)
docs/spec/        how the game behaves, written from a reading of it, one file a subject
docs/             status.md (the state in full, every placeholder), manual.md (keys, options, tools,
                  recording), findings.md (what was learned, the traps), workbench.md (recordings and
                  scratch scripts), history.md (the log and the user's decisions)
local/            machine-specific settings, tools, traces and scratch output, never committed
```

## What was learned, and how

This is a re-implementation, as OpenMW is of Morrowind's engine or OpenRCT2 of RollerCoaster Tycoon 2: it needs the original game to run, and contains none of it.

**What you need to own.** A legally obtained copy of Escape from Tarkov, with Single Player Tushonka (SPT) installed over it; optionally S.T.A.L.K.E.R. Anomaly, with or without GAMMA. We do not condone piracy.

**What is not here.** No models, textures, animations, sounds, maps, game code or decompiled code from either game, in the files or in their history. Everything the app shows is read from your own installs, in place, each time it starts. The only thing it writes from them is the inventory's pictures, which it draws itself and keeps in your own `local/` folder.

**How the behaviour was learned**, plainly:

- **From the game's code.** The author decompiled Escape from Tarkov's code (with ILSpy) on their own machine, from their own install, and read it. What each part of the controller does (its rules, formulas, figures, state machines and the order things happen in) was written down in our own words in `docs/spec/`, and the Rust here was written from those notes, not translated from the decompiled code. The decompiled code was never committed.
- **Names, so a reader can find the place.** The notes name the game's own classes and fields where the behaviour was read (`ProceduralWeaponAnimation`, `PreviewPivot`). One file lists names and nothing else: `crates/trp-data/src/hard_settings_layout.txt`, the kinds and names of the fields of the game's settings object, which is stored without a description of itself and cannot be read without one.
- **Figures.** Constants are read from your install when the app starts. The few written into the code (for a fallback, or where a figure cannot be read) each name where they came from; guesses are marked `ESTIMATE` and are listed in `docs/status.md`.
- **From the running game.** A BepInEx plugin of our own for SPT (`hosts/spt-oracle`) recorded what the game's character, camera and weapon do frame by frame, and the simulation was compared with those recordings ("Compared with" in the table above). The recordings are not included.
- **File formats.** Unity's from the files themselves (bundles describe their own layouts) and the public documentation of open tools (AssetRipper, AssetStudio, UnityPy). S.T.A.L.K.E.R.'s models and packed archives from that engine's published source (OpenXRay), the archives' compression written afresh from the published LZHUF and LZO algorithms. Binary FBX as Blender's developers have written it up.

**Not affiliated** with or endorsed by Battlestate Games, GSC Game World, or the teams of Single Player Tushonka (SPT), Anomaly or GAMMA. Escape from Tarkov and S.T.A.L.K.E.R. are trademarks of their owners, named here only to say which games this works with.

**License**: Apache-2.0 (`LICENSE`, at the repository's top), for this project's own code only; it grants nothing in the games it reads.
