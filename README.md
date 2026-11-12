# TRP (Tushonka Runtime Project)

TRP rebuilds the first-person character controller of Escape From Tushonka in Rust, on the Bevy engine. It reads everything it shows from your own Single Player Tushonka (SPT) install each time it starts. No file of any game is in this repository.

It is a re-implementation in the way OpenMW is of Morrowind's engine: it needs the original game to run and contains none of it.

## Requirements

- Windows 10 or 11, and a desktop graphics card.
- Your own copy of Escape From Tushonka with SPT 4.1 installed over it ([installer](https://sp-mod.com/installer)).
- Optional: S.T.A.L.K.E.R. Anomaly or GAMMA, for the outfits read from that game.
- Rust (stable, MSVC) and the Visual Studio C++ build tools. `Install.bat` offers to install both.

## Install and play

1. Download this repository (**Code > Download ZIP**) and unzip it, or clone it with git.
2. Double-click `Install.bat` once. It installs what is missing, builds TRP, and asks where your game folders are. The first build takes several minutes and about 10 GB of disk.
3. Double-click `Play.bat` to play. After an update it rebuilds by itself.

You start on a test range. The tables along the wall hold every weapon of your install. `~` shows the debug menu.

## What is in it

| Folder | What it holds |
|---|---|
| `TRP/crates/trp-data` | Readers for the install: Unity bundles, the SPT database, animators, screens and sounds. |
| `TRP/crates/trp-sim` | The controller as a plain simulation, with no engine in it. |
| `TRP/crates/trp-physics` | PhysX 4.1 set up as the game sets it up, for ragdolls and loose things. |
| `TRP/crates/trp-app` | The Bevy application: the range, the weapons, the screens. |
| `TRP/crates/trp-tools` | The probe: a command-line tool that looks into the install. |
| `TRP/docs` | The feature list, the manual, and the specs of how the game behaves. |

Start with [`TRP/docs/features.md`](TRP/docs/features.md) for what is built and [`TRP/docs/manual.md`](TRP/docs/manual.md) for how to run and inspect it. [`TRP/docs/index.md`](TRP/docs/index.md) maps every source file. Contributors and coding agents should read [`AGENTS.md`](AGENTS.md).

## How the behaviour was learned

- **From the game's code.** The game's code was decompiled on the author's own machine from the author's own install, and read. What each part does was written down in our own words in `TRP/docs/spec/`. The Rust here was written from those notes, not translated from the decompiled code, which was never committed.
- **From SPT's published source**, where SPT's server decides a thing rather than the game.
- **From the running game.** A recorder plugin, which is not in this repository, wrote what the game's character, camera and weapon do frame by frame, and the simulation was compared with those recordings. Each spec says what it was compared with.
- **Figures** are read from your install when the app starts. The few written into the code name their source, and a guess is marked `ESTIMATE`.
- **The physics engine** is NVIDIA's PhysX 4.1, built from NVIDIA's published source through the `physx-sys` bindings.
- **File formats.** Unity's from the files themselves and the documentation of open tools (AssetRipper, AssetStudio, UnityPy). S.T.A.L.K.E.R.'s from OpenXRay's published source. FMOD sound banks as the vgmstream project documents them. Binary FBX as Blender's developers describe it.
- **Screen effects** were read from the compiled shaders in your install, disassembled with the Windows SDK, and written down as sums in the specs. The shaders here were written from those notes.

One file lists names and nothing else: `TRP/crates/trp-data/src/setup/hard_settings_layout.txt` holds the kinds and names of the fields of the game's settings object, which cannot be read without them.

## Licence

Apache-2.0 (`LICENSE`) for the code and the documents. It grants nothing in the games TRP reads. You need a legally obtained copy of each game. We do not condone piracy.

An unofficial fan project, not affiliated with or endorsed by Battlestate Games, GSC Game World, or the teams of Single Player Tushonka, Anomaly or GAMMA. S.T.A.L.K.E.R. is a trademark of its owner.
