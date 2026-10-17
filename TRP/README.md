# TRP (Tushonka Runtime Project)

A Rust rewrite, on Bevy, of the first-person character controller of the game Single Player Tushonka (SPT) runs on. It reads everything it shows from your own SPT install at startup. This repository contains code only: no models, textures, animations, sounds or game code. (One thing it can show is not the install's: a weapon of one's own, whose model lies outside the repository in `local\`.)

This is an unofficial fan project for study. It is not affiliated with or endorsed by Battlestate Games or GSC Game World. You need your own copies of the games to run it: see "What was learned, and how" at the end.

**This file is the short form.** The state in full, what is missing and every placeholder: `docs/status.md`. Every key, every option, every tool and how to record and compare: `docs/manual.md`. How the game behaves: `docs/spec/`.

## Status

The first-person controller is built and, wherever a recording can show it, compared with traces recorded from the running game: sixteen scripted recordings, forty-one made by hand, and fifty-four more over nine courses (2026-10-07 to 2026-10-15). **"Compared with" below is what "matches" means here; "nothing" means from the code and the install alone.**

| What | Spec (`docs/spec/`) | Compared with the running game |
|---|---|---|
| Reading the install: bundles, meshes, clips, animators, the database, the settings, the screens | | every test runs on it |
| The body's animator (22 layers), the body drawn, worn items | `body-animator.md` | its state on every frame of 60 scenarios (70,700 frames); the legs bone for bone, 0.2 mm |
| Movement, the character's own collision | `movement.md` | the movement state on every frame; distances within 4.4 mm; the eye within a millimetre or two |
| Vaulting and climbing | `vaulting.md` | vaults within 6 mm, the climb within 3 cm |
| Stamina | `stamina.md` | to four decimal places, the legs run dry included |
| Weapon motion, breathing, recoil, the camera's box, the turn-away | `weapon-handling.md` | 179 shots: the same least and most, means within 1.5% |
| Firing and reloading by the weapon's own animator | `firing.md` | the animator's state and time on 461,415 frames; the hand markers within 0.05 mm |
| The arms' solver | `hands-on-weapon.md` | 0.02 degrees and 0.2 mm, given the game's goals |
| Every kind of weapon the install has ("The weapons", below) | `shotgun.md`, `pistol.md`, `bolt-action.md`, `more-weapons.md` | each but the M32A1 on every frame of a recording |
| The inventory, looting, its clicks and keys, the menu of a right click; its pictures, drawn by a copy of the game's icon camera | `inventory.md`, `icons.md` | places and weights to the gram; sixteen recorded moves of sixteen; the menu and the pictures: seen |
| Doors, containers, a trunk, kicks, a lock and its key, the left hand's gestures | `interactions.md` | thirty recordings by hand: angles to 0.000 degrees, gestures within a frame |
| The keys as the game's input manager hears them; the options tab | `settings.md` | the keys on the game's own frame; the options: nothing |
| The head's free look (middle mouse) | `free-look.md` | a recording by hand: the head's turn, the mouse's hold and the view's turn on every frame |
| The character as others see it (`F5`) | `third-person.md` | twelve recordings from outside: the states on every frame, the arms within 0.1 to 1.4 degrees, the reach for the vest within 0.013 |
| Empty hands | `inventory.md` | appearing: by hand; the rest: nothing |
| A weapon of one's own (the StG 44, on the FN SCAR-L's animations); the catalogue of donors | `custom-weapons.md` | its donor, frame for frame |
| **Outfits from another game**: S.T.A.L.K.E.R.'s Sunrise and SEVA suits as the body, with that game's own first-person arms | `outfits.md` | nothing (neither game shows it); seen beside Tarkov's hands |
| **Health**: seven parts, bleeding, fractures, pain, hunger and thirst, falls, barbed wire; medicine, food and drink used with their own hands; death, and `Enter` to begin again | `health.md` | fifteen recordings to the frame and to 0.01 point; the items' twelve uses on the game's frames |
| **The game's own physics engine**, PhysX 4.1 set up as the game's: the corpse, things dropped and thrown | `physics.md` | the corpse within 1.5 to 3.5 cm of four recorded deaths; things thrown: nothing |
| **`TARKOV UI`**, a switch: the game's own screens, panels, texts and sounds, read from the install | `inventory.md`, `sound.md` | GEAR and HEALTH beside the user's captures; the sounds and texts beside a recording out of a raid; in play: nothing |
| **The sounds of moving**: footsteps by the surface underfoot (the game's twenty), sprint, jump, landing, crawl, turn, kit; a sampler of every surface (`--start surfaces`) | `footsteps.md` | nothing of the game's |
| **The hands' sounds**: what a weapon, an item used and the empty hands sound as their animation marks it, from each one's own sound player; the sights brought up (the kit, the weapon's aiming clip); Vorbis clips read with FMOD's headers from the game's engine | `sound.md` | nothing of the game's; every Vorbis clip decodes to its own length |
| **The shots' sounds**: each weapon's own banks, an automatic weapon's fire as the game's recorded loop ended on the next beat by its tail, any other's each shot whole; the game's own audio mixer read (its master compressor on the mix) | `sound.md` | nothing (no recording of a shot with sound) |
| The game's texture quality, mip streaming and shadows | `streaming.md`, `settings.md` | nothing; 48 rifles: 1,970 MB of pictures on the card without streaming, 232 MB with |

381 tests pass. The regression over the sixteen kept recordings is run after every change to the simulation.

**Not built** (in full: `docs/status.md`, "Missing"): skills as a system; the world's sounds; muzzle effects, shell ejection; malfunctions, ballistics and dispersion, optics, mounting, blind fire, the left shoulder; grenades and the knife; the rocket launcher; slopes and sliding; searching item by item; a second character; Tarkov's own shading (materials are an approximation); the STALKER host.

**Placeholders.** Every guess in the code is marked `ESTIMATE` and every unconfirmed rule `PROVISIONAL`; all are listed in `docs/status.md`, "Placeholders". Those that show most: the test range and its crates, which are the app's own; the StG 44's round, recoil and size.

## Requirements

- Windows, Rust (stable, MSVC toolchain) with the Visual Studio C++ build tools: `Install.bat` offers to install both.
- Single Player Tushonka (SPT) 4.1, installed over your own copy of the game (its client 0.16.9). SPT and the mods for it are on its Forge (`sp-tushonka.com`). The item database is read from SPT's files, so the game alone is not enough.
- A GPU that supports BC texture compression (any desktop GPU).
- Optional, for the suits from S.T.A.L.K.E.R.: Anomaly, alone or under GAMMA. Both are read, never written; without them the suits are items with nothing drawn for them.

## Running

**`Install.bat`**, at the repository's top, once: it checks for the C++ build tools and Rust and offers to install what is missing (through winget, asking first), builds the app in release, and asks where your games are. **`Play.bat`** then starts it (`cargo run --release -p trp-app`, rebuilt first only when the code changed), passing on anything after it (`Play.bat --equip`). By hand, from this folder:

```
cargo run --release -p trp-app
```

**The first start asks where your games are** (`Install.bat` asks it; a run that finds no install asks it in its console): your SPT folder, offered for Enter where it lies in a usual place (`C:\SPT`, `D:\Games\SPT`), and, if you have it, S.T.A.L.K.E.R.: GAMMA's folder or Anomaly's (Enter skips). The answers are checked and kept in `local/` (`eft_dir.txt`, `gamma_dir.txt`, `anomaly_dir.txt`); `--setup` asks again, and `EFT_DIR`, `GAMMA_DIR` and `ANOMALY_DIR` do instead. The first line printed is the version (`--version` prints it alone; `docs/manual.md`, "Versions").

The first build compiles Bevy and PhysX 4.1 from its source: in release, under three minutes here and a build folder of 2.1 GB (the workspace in debug, `cargo build --workspace`, 4.1 GB, 5.1 GB with the tests). With Rust (1.5 GB), the C++ Build Tools and Windows SDK (5.1 GB) and the crates downloaded (about 1 GB), about 10 GB from nothing, besides SPT; the repository itself is 7 MB. The character starts bare on a test range; **the crate straight ahead holds a rifle, a vest and a backpack, and the tables along the low wall hold every weapon of the install** by class (look at one and press `F`; Alt and a click puts a thing on; `Tab` shuts the screen; `2`, `3` or `1` takes the weapon in hand).

**The keys are the install's own bindings** (`Control.ini`); `F1` shows the guide, and the inventory's last tab, `OPTIONS`, changes them. As the install has them:

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
| `R`, `R` twice | Reload from the fullest magazine worn; the quick reload, the old magazine let fall |
| `Left Alt` + `T`, `Left Shift` + `T` | Check the magazine; look into the chamber |
| `B`, `B` twice, `Left Alt` + `B` | Next fire mode; the fullest; look at the selector |
| `L`, `Left Alt` + `L` | Look the weapon over; fold the stock |
| `2` / `3` / `1` | The weapon on the sling, on the back, in the holster |
| `F`, wheel with a menu up | Do the menu's chosen line for what is looked at; choose a line |
| `Tab` | The inventory (Ctrl and a click sends a thing across, Alt and a click puts it on, a double click uses or opens it, the right button its menu, `Delete` puts it down, `4` to `0` bind it) |
| `Escape` | Shut the window on top, or the screen |
| `F1`, `F4`, `F5` | The app's panel; a free camera; play while watched from outside |
| `F7`, `F8` | Be hurt; die (`Enter` begins again) |

**Options most used** (all of them: `docs/manual.md`, "Running"):

```
Play.bat --weapon rhino_50ds          begin with a weapon in hand, by its id or a part of its name
Play.bat --equip                      begin with the crate's rifle, vest and backpack on
Play.bat --view watch                 watched from outside (also fp, side, body, map)
Play.bat --tap 200:lmb:6 --tap 420:r:6 --screenshot local\shots\x.png --screenshot-frame 700
```

(From the repository's top; from this folder, `cargo run -p trp-app -- <options>` does the same in debug.)

The last is a capture: keys pressed on given frames through the bindings, one frame saved, and the run ends.

## The weapons

Every weapon of the install with a stock preset (148), and the one of one's own, lies in a gun case on the tables of weapons, by class, with what loads it in a box under it; a ninth table, `MEDICAL`, holds every medical item, food and drink. Barbed wire before the knee-high wall and a ledge 6 m up hurt (`--start wire`, `--start ledge`). Which logic a weapon runs on goes by its template (`docs/spec/more-weapons.md`).

| Weapon | Item | Kind | Compared with |
|---|---|---|---|
| Kalashnikov AKM | `59d6088586f774275f37482f` | a magazine that comes out; single shots and automatic | fourteen recordings |
| Benelli M3 Super 90 | `6259b864ebedf17603599e88` | a tube filled a shell at a time; pumped or self-loading; a folding stock | the sixteenth recording |
| FN Five-seveN | `5d3eb3b0a4b93615055e84d2` | a pistol; the slide held back | a recording by hand, every frame to a weapon switch |
| Remington Model 700 | `5bfea6e90db834001b7347f3` | a bolt action, its magazine filled with the bolt open | a recording by hand, every frame |
| Izhmash SV-98 | `55801eed4bdc2d89578b4588` | a bolt action whose magazine comes out | a recording by hand, every frame |
| TOZ SKS | `574d967124597745970e7c94` | a self-loader filled through its action | a recording by hand, every frame |
| Chiappa Rhino 50DS | `61a4c8884f95bc3b2c5dc96f` | a revolver, single and double action | two recordings by hand, every frame |
| MP-43e-1C | `5580223e4bdc2d1c128b457f` | two barrels loaded one by one, fired singly or together | a recording by hand, every frame to a weapon switch |
| Milkor M32A1 | `6275303a9f372d6ea97f9ec7` | a launcher with a drum a spring turns (its grenade does not fly) | nothing |
| Sturmgewehr 44 | `66f0a5740000000000000044` | not the game's: a model of one's own on the FN SCAR-L's animations (`custom/stg44.json`) | its donor |

A weapon found in a case has a full magazine, tube or cylinder and no round chambered; beside it lie spare magazines or rounds, which the reloads take from the pockets and the vest worn.

**Outfits**: the crate holds S.T.A.L.K.E.R.'s Sunrise and SEVA suits, read from your GAMMA folder (worn on the body armour's slot, each is the character's body with that game's own first-person arms on Tarkov's animations; `CUSTOM FIRST PERSON HANDS` in the options switches the arms off), and Tarkov's Telnik to set beside them (`docs/spec/outfits.md`).

## Looking into the install, and comparing with the game

```
cargo run -p trp-data --bin probe -- weapon 59d6088586f774275f37482f          a weapon as assembled
cargo run -p trp-sim --bin replay -- --traces local\traces\recording13 06     a recording replayed, gap by value
cargo run -p trp-sim --bin byhand -- local\traces\byhand3                     a recording made by hand, read
```

`hosts/spt-oracle` is the recording plugin, a BepInEx plugin for SPT that plays scripted input in a raid, or records what is done by hand, and writes what the game's own controller does frame by frame; `replay` and `byhand` set the simulation beside its recordings. It is the project's research tool and stays in the private repository: neither it, its scenarios nor the recordings are in the public copy, and nothing of them is needed to build or run the app. Every probe and tool: `docs/manual.md`.

## Tests

```
cargo test --workspace
```

The unit tests need nothing; those in each crate's `tests/` folder run against the configured install and pass with a note when there is none (`--nocapture` prints what they measured); a few need the recordings in `local/traces` too.

## Layout

```
crates/trp-data    readers for the install (Unity bundles, the SPT database, animators, the settings
                   object, the screens), binary FBX, S.T.A.L.K.E.R.'s models; bin/probe
crates/trp-sim     the controller as a plain simulation with no engine in it; bin/replay, bin/byhand
crates/trp-physics the game's physics engine (PhysX 4.1) set up as the game's; bin/corpse
crates/trp-app     the Bevy application: the range, the rigs, the screens
hosts/spt-oracle   the recording plugin for SPT (C#) and its scenarios (private repository only)
custom/            the weapons and outfits that are not the install's (their models outside the repo)
docs/              spec/ (how the game behaves), status, manual, decisions, findings, workbench, history
local/             machine-specific settings, tools, traces and scratch output, never committed
```

## What was learned, and how

This is a re-implementation, as OpenMW is of Morrowind's engine or OpenRCT2 of RollerCoaster Tycoon 2: it needs the original game to run, and contains none of it.

**What you need to own.** A legally obtained copy of Escape from Tarkov, with Single Player Tushonka (SPT) installed over it; optionally S.T.A.L.K.E.R. Anomaly, with or without GAMMA. We do not condone piracy.

**What is not here.** No models, textures, animations, sounds, maps, game code or decompiled code from either game, in the files or in their history. Everything the app shows is read from your own installs, in place, each time it starts. The only thing it writes from them is the inventory's pictures, which it draws itself and keeps in your own `local/` folder.

**How the behaviour was learned**, plainly:

- **From the game's code.** The author decompiled Escape from Tarkov's code (with ILSpy) on their own machine, from their own install, and read it. What each part of the controller does (its rules, formulas, figures, state machines and the order things happen in) was written down in our own words in `docs/spec/`, and the Rust here was written from those notes, not translated from the decompiled code. The decompiled code was never committed.
- **From SPT's published source.** Where SPT's server, not the game, decides a thing (the magazine of a weapon whose stock build has none), SPT's open source was read, and its own documentation in the install.
- **Names, so a reader can find the place.** The notes name the game's own classes and fields where the behaviour was read (`ProceduralWeaponAnimation`, `PreviewPivot`). One file lists names and nothing else: `crates/trp-data/src/hard_settings_layout.txt`, the kinds and names of the fields of the game's settings object, which is stored without a description of itself and cannot be read without one.
- **Figures.** Constants are read from your install when the app starts. The few written into the code (for a fallback, or where a figure cannot be read) each name where they came from; guesses are marked `ESTIMATE` and are listed in `docs/status.md`.
- **From the running game.** A BepInEx plugin of our own for SPT (`hosts/spt-oracle`, kept in the private repository) recorded what the game's character, camera and weapon do frame by frame, and the simulation was compared with those recordings ("Compared with" in the table above). The plugin, its scenarios and the recordings are not published.
- **The physics engine.** The game's is NVIDIA's PhysX 4.1, whose source NVIDIA publishes (BSD-3-Clause); it is built here from that source through Embark Studios' bindings (`physx-sys`). How the game's Unity sets it up was read from the install's own settings and the game's code, and where neither says, settled against the recordings (`docs/spec/physics.md`).
- **File formats.** Unity's from the files themselves (bundles describe their own layouts) and the public documentation of open tools (AssetRipper, AssetStudio, UnityPy). S.T.A.L.K.E.R.'s models and packed archives from that engine's published source (OpenXRay), the archives' compression written afresh from the published LZHUF and LZO algorithms. Binary FBX as Blender's developers have written it up. FMOD's sound banks (FSB5) as the open vgmstream project documents them; a Vorbis clip's setup header, which FMOD leaves out of the bank, is read from your own copy of the game's engine each time, and the clip decoded by `lewton`.

**Not affiliated** with or endorsed by Battlestate Games, GSC Game World, or the teams of Single Player Tushonka (SPT), Anomaly or GAMMA. Escape from Tarkov and S.T.A.L.K.E.R. are trademarks of their owners, named here only to say which games this works with.

**License**: Apache-2.0 (`LICENSE`, at the repository's top), for this project's own code only; it grants nothing in the games it reads.
