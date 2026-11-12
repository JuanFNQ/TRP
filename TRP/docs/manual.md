# Manual

How to run TRP, what the keys and flags do, and how to look into the install and compare with the game. Commands run from `TRP/`.

## Running

`Install.bat` sets everything up once and `Play.bat` plays. By hand:

```
cargo run -p trp-app
```

The first build compiles Bevy and PhysX and takes several minutes. `--version` prints the version and stops.

**The first start.** Where no install is named, the app asks in its console for three folders: SPT (needed), then GAMMA's Mod Organizer folder and Anomaly's folder (optional, Enter for none). The answers are kept in `local/spt_dir.txt`, `local/gamma_dir.txt` and `local/anomaly_dir.txt`, in the `local` folder beside `Cargo.toml`, with the options and the logs, wherever under the workspace the app is started from. `--setup` asks again. The `SPT_DIR` environment variable overrides the kept SPT folder.

**Logs.** Every run writes `local/logs/run_<date>_<time>.txt` with the versions, the paths, the graphics card and everything printed. A crash writes `crash_<date>_<time>.txt` with the backtrace. Both hold your folder names, so look one over before sharing it.

## Keys

The keys are your install's own bindings. `F1` shows them in the app and the CONTROLS page rebinds them. These are the defaults.

| Key | Action |
|---|---|
| `W A S D`, mouse | Move, look |
| `Left Shift` | Sprint |
| `Space` | Jump. Held with `W`: vault or climb. |
| `C`, `X` | Crouch, lie down |
| `C` + wheel, wheel | Stance height, walking speed |
| `Caps Lock` | Walk |
| `Q` / `E` | Lean. With `Left Alt`: step aside |
| `Left Alt` + `A` / `D` | Lean slowly |
| Middle mouse | Free look |
| Left mouse, right mouse | Fire, aim |
| `Left Ctrl` + right mouse | Next sight |
| `Left Alt` + right mouse | Next scope mode |
| `Left Alt` + wheel | Zoom a variable scope |
| `Page Up` / `Page Down` | Zeroing |
| `Left Alt` | Hold breath |
| `R`, `R` twice, `R` held + wheel | Reload, quick reload, pick a magazine |
| `Left Alt` + `T`, `Left Shift` + `T` | Check the magazine, check the chamber |
| `Left Alt` + `R`, `Left Ctrl` + `R` | Unload the magazine, unload the chamber |
| `B`, `B` twice, `Left Alt` + `B` | Next fire mode, fullest fire mode, check the selector |
| `L` | Inspect the weapon |
| `Left Alt` + `L` | Fold the stock |
| `Alt` + `W` / `Alt` + `S` | Blind fire over the top / to the right |
| `Mouse4` | Left shoulder |
| `V`, `Ctrl` + `V` | Mount the weapon, toggle the bipod |
| `T`, `Ctrl` + `T`, `H` | Tactical device, next device, head light |
| `N` | Face shield or night vision |
| `2` / `3` / `1` | Weapon on the sling, on the back, in the holster. `2` or `3` again: the underbarrel launcher |
| `1` twice | Quick draw of the pistol |
| `G`, `U` | Grenade, knife |
| `4` to `9`, `0` | Quick slots |
| `F` | Use what is looked at. The wheel picks the menu's line |
| `Tab` | Inventory |
| `Z` twice | Drop the backpack |
| `Y`, `F1` to `F12` | Gestures and phrases |
| `` ` `` | The debug panel |
| `Enter` | After a death: start again |

In the inventory: `Ctrl` + click sends a thing to the other side, `Alt` + click puts it on, a double click opens or inspects it, `Delete` drops it, and a right click opens its menu.

## Options

The options tab is the inventory's last tab. Its pages are GRAPHICS, SOUNDS, HUD, CONTROLS, FEATURES and SKILLS. Settings are kept in `local/options.json`. `docs/features.md` lists every feature and its switch.

- **FEATURES** holds the experimental switches, then SIMPLE XP GAIN and the character's name under CHARACTER, then the range's time of day. A switch takes effect at the next start.
- **SKILLS** sets each skill's level and each weapon family's mastery.
- **RESET TO DEFAULT** on a page puts that page back as it comes.

## Command-line flags

An unattended run (any of `--screenshot`, `--tap`, `--hold`, `--scenario`) reads no options file and ignores the mouse.

| Flag | What it does |
|---|---|
| `--weapon <id or part of a name>` | The weapon the run begins with |
| `--add-weapon <id or name>` | Lays another weapon in its gun case |
| `--preset <id or file>` | The weapon as another build |
| `--equip` | Start with the rifle, vest and backpack on |
| `--bare`, `--no-spare-magazines` | Start with nothing, or without spare magazines |
| `--put-on <id or file>`, `--wear <id>`, `--carry <id>` | Wear a thing in its slot, over the clothing, or carry it |
| `--loadout <edition>/<side>`, `--suit <id>` | A starting kit and clothing from the database |
| `--lay <id>`, `--lay-ahead <m>` | Lay a thing on the floor ahead |
| `--weight <kg>`, `--strength <n>`, `--endurance <n>` | The character's load and two skills |
| `--skill <Name=level>`, `--weapon-skill <n>`, `--mastering <0-2>`, `--experience <n>` | Skills, mastery and experience |
| `--start <place>` | Where the run begins, for example `open`, `lanes`, `ledge`, `stairs`, `doors`, `workbench`, `dummy`, `rifles` and `surfaces` |
| `--yaw <deg>`, `--pitch <deg>` | Where the view points at the start |
| `--view fp\|side\|body\|watch\|map`, `--pov first\|third` | The camera, and which pose the character takes |
| `--hour <h>`, `--level <file>` | The range's time of day, and a range read from a file |
| `--inventory`, `--loot`, `--search-body` | Start with the inventory, a crate or a body open |
| `--feature <key>=on\|off` | Throw a switch for this run |
| `--hold <keys>` | Hold keys for the whole run, for example `w+shift` or `fire+aim` |
| `--tap <frame>:<key>[:<frames>]` | Press a key on a frame |
| `--mouse`, `--move`, `--drag`, `--drag-at`, `--wheel`, `--scroll`, `--zoom` | Move, drag and scroll the mouse on a frame |
| `--damage <frame>:<part>:<amount>`, `--poison ...`, `--malfunction`, `--heat`, `--repair` | Hurt the character or wear the weapon on a frame |
| `--screenshot <file>`, `--screenshot-frame <n>`, `--screenshot-crop x,y,w,h` | Write a picture of a frame, or of a region of it |
| `--resolution WxH`, `--fov <deg>`, `--msaa`, `--ssao`, `--shadow-quality`, `--texture-quality`, `--dust-resolution`, `--backend`, `--render-thread on\|off`, `--fps-limit` | Graphics settings for the run |
| `--frame-rate <n>`, `--fixed-dt <ms>`, `--seed <n>`, `--clock HH:MM` | Make a run repeat exactly |
| `--log-frames`, `--log-pacing <file>`, `--log-sounds`, `--dump-ui <file>` | Write what a run does to a file |
| `--scenario <file>` | Play one of the recorder's scripts in the app |
| `--export-level <file>` | Write the range as a file to edit in Blender |
| `--setup`, `--setup-only` | Ask for the install folders again |

`--tap` names a key by a word: a letter, a digit, `num0` to `num9`, `numenter`, `tab`, `delete`, `backspace`, `space`, `ctrl`, `alt`, `shift`, `caps`, `enter`, `escape`, `backquote`, `f1`, `f7` to `f9`, `pageup`, `pagedown`, `home`, `end`, the arrows `left`, `right`, `up` and `down`, and the mouse's `lmb`, `rmb`, `mmb`, `mouse3` and `mouse4`. A key held with another is a longer tap begun before it: `--tap 100:ctrl:6 --tap 102:a:1` is Ctrl+A. A letter, a digit, the space and the keys that edit a text field are also typed into a field that has the keys.

A flag that takes a file takes it as its next argument. Give one, or the flag after it becomes the file's name.

## Looking into the install

The probe reads your install and prints what it finds. Use it before taking any figure from the game.

```
cargo run -p trp-tools --bin probe -- weapon <template id>          a weapon as assembled
cargo run -p trp-tools --bin probe -- states <template id> reload   an animator's states, clips and marks
cargo run -p trp-tools --bin probe -- hardsettings <word>           the game's settings object
cargo run -p trp-tools --bin probe -- find <word>                   a bundle's objects by name
```

It has about a hundred commands. Each is an arm of the match in `crates/trp-tools/src/bin/probe`.

## Comparing with the game

The specs say which parts were compared with recordings of the running game, and which were only read from its code. The recorder and the tools that read its recordings are not in this copy.

## Other tools

| Command | What it does |
|---|---|
| `bash tools/index.sh` | Rewrites `docs/index.md` |
| `bash tools/section.sh <spec> [heading]` | Lists a spec's headings, or prints one section |
| `bash tools/section.sh --find <word>` | Finds a word in every document |

## Versions

One number covers the whole of TRP: `version` under `[workspace.metadata.trp]` in `Cargo.toml`. Raise the middle number when something new can be done or seen, and the last number for anything else.
