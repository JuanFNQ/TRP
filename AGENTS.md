# TRP (Tushonka Runtime Project)

TRP rebuilds Escape From Tushonka's first-person character controller in Rust on Bevy. It reads all game data at runtime from the player's own Single Player Tushonka (SPT) install, so the repository holds code and documents only. It is meant to be hosted by other games, and STALKER GAMMA is the first intended host. "Matches Tarkov" means measured against a recording of the real game.

## Layout

The Cargo workspace is `TRP/`. Run every command from there.

| Path | What it holds |
|---|---|
| `crates/trp-data` | Reads the install: Unity bundles, the SPT database, animators, screens, sounds. |
| `crates/trp-sim` | The controller as a plain simulation with no engine types. |
| `crates/trp-physics` | PhysX 4.1 set up as the game sets it up: ragdolls and loose things. |
| `crates/trp-app` | The Bevy app (`trp.exe`): the range, the weapons, the screens. |
| `crates/trp-tools` | The probe: a command-line tool that looks into the install. |
| `custom` | Figures for weapons and outfits that are not the install's. |
| `docs` | `index.md`, `features.md`, `manual.md` and `spec/`. |
| `tools` | `index.sh` writes the index. `section.sh` reads one section of a long spec. |
| `local` | Machine settings, logs and scratch. Ignored by git. |

Each crate's `src/` is split into feature folders, for example `trp-app/src/screens` and `trp-sim/src/weapons`. A feature that needs more than one file is a folder with a `mod.rs` that names its parts.

## Rules

1. **Research before code.** Prefer a documented format, then published source, then decompilation.
2. **The installs are inputs.** Never write into the SPT, Anomaly or GAMMA folders.
3. **No game content in the repository.** No assets, extracted files, decompiler output or pasted game code. Scratch goes in `local/`.
4. **Spec, then implement.** Write the game's behaviour into `docs/spec/` in your own words, then write original code from that.
5. **Every constant has a source:** the install or the spec. Mark a guess `ESTIMATE` and an unconfirmed rule `PROVISIONAL` in the code.
6. **Claims are measured.** Say what was compared with a recording, what was only read from code, and what was not run.
7. **Do not depart from Tarkov.** Where the game's code or a recording shows what the game does, do that. The one liberty is how results are drawn between frames, and it must not change the simulation's results.
8. **Ask first** before launching a game, installing a toolchain, committing or publishing.

## Commands

```
cargo build --workspace
cargo test --workspace
cargo run -p trp-app                                  the app (keys and flags: docs/manual.md)
cargo run -p trp-tools --bin probe -- <what> ...      look into the install
bash tools/index.sh                                   rewrite docs/index.md
```

The install's path comes from `SPT_DIR` or `local/spt_dir.txt`. A run writes its log to `local/logs/`.

## Finding things

- **A file:** search `docs/index.md` for a word. It lists every source file with one line on what it holds.
- **A feature:** `docs/features.md` names its spec and its switch.
- **A rule of the game:** `bash tools/section.sh <spec>` lists a spec's headings, and with a heading it prints that section. `bash tools/section.sh --find <word>` searches every document. The specs are long, so read a section and not the whole file.
- **A figure in the install:** use the probe before reading decompiled code.

## Changing things

1. Find the files in the index and read the matching section of the spec.
2. If the behaviour changes, change the spec first.
3. Keep a source file under about 500 lines. A file holds one piece of a feature and opens with a `//!` sentence that says what it holds. When a file outgrows the limit, turn it into a folder and split it by meaning.
4. Write a helper once. Search the index before adding one.
5. A new system gets a switch in `Features` (`trp-app/src/screens/options_view`). Only a switch a player has a reason to use goes on a menu page.
6. Run the tests. The recorder and its recordings are not in this copy, so say which of your changes could not be compared with the game.
7. Raise the version (`[workspace.metadata.trp]` in `Cargo.toml`), add a line to `CHANGELOG.md`, and run `bash tools/index.sh`.

## Working notes

- Source files use Windows line ends (CRLF). Keep them.
- In PowerShell 5.1, do not put `2>&1` on cargo, and do not write a source file with `Set-Content` because it adds a byte-order mark.
- If cargo cannot replace `trp.exe`, the app is open. Wait for it to close.
- Bevy is pinned to 0.18 and `physx-sys` to 0.8.2 (PhysX 4.1).
- In decompiled code, check that something creates or calls a class before taking a rule from it.
- Look at a screenshot before judging a rendering change (`--screenshot`, in the manual).
- A helper of a Bevy system takes a resource as `Mut<T>` (`resource.reborrow()`), never as `&mut T`. A plain reference marks the resource changed on every call, and whatever watches it then runs every frame.
- What is hidden for long is parked (`platform/park`): it leaves the engine's work until shown. A system that must keep a parked thing's state adds `EvenParked` to its query.

## Status

Version 0.262.2. `docs/features.md` lists what is built and what is not.
