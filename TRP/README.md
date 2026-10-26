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
| Movement, the character's own collision, slopes and sliding, the slide out of a sprint, the room a step aside, a roll, lying down, standing up and the crawl need, the head kept out of walls, the speed limits from armour and from being hit | `movement.md` | the movement state on every frame; distances within 4.4 mm; the eye within a millimetre or two; the slope's rules on two recordings by hand to the sixth place (the slide out of a sprint, the room's refusals: from the code); the head's capsule placed to 0.002 mm standing on 13,299 frames, its limit exact; the armour's speed limit on 327 recordings to the sixth place |
| Vaulting and climbing | `vaulting.md` | vaults within 6 mm, the climb within 3 cm |
| Stamina, a wound's burn, the pistol's quick draw | `stamina.md` | to four decimal places, the legs run dry included (a wound and the quick draw: from the code) |
| Weapon motion, breathing, recoil, the camera's box, the turn-away, a heavy weapon's aim sway | `weapon-handling.md` | 179 shots: the same least and most, means within 1.5%; the M60E6's aim-in: its turn 0.76 degrees rms, the eye 0.3 mm (the sway's random direction fitted); its held aimed fire with the game's recoil draws: the camera within 0.7 mm, the eye's height against the scope 0.22 mm rms |
| Firing and reloading by the weapon's own animator, the weapons with no chamber fired off their magazines | `firing.md`, `more-weapons.md` | the animator's state and time on 461,415 frames; the hand markers within 0.05 mm; the M60E6's rounds and states on every frame of ten scripts |
| The arms' solver | `hands-on-weapon.md` | 0.02 degrees and 0.2 mm, given the game's goals |
| Every kind of weapon the install has ("The weapons", below) | `shotgun.md`, `pistol.md`, `bolt-action.md`, `more-weapons.md` | each but the M32A1 on every frame of a recording |
| The inventory, looting, its clicks and keys, the menu of a right click; its pictures, drawn by a copy of the game's icon camera | `inventory.md`, `icons.md` | places and weights to the gram; sixteen recorded moves of sixteen; the menu and the pictures: seen |
| A face shield on the head: its switch (`N`, the menu), the gestures, its hinge (on the helmet or the shield), its icon, and its mask over the first person's view (scratches, the blur at the edges, a frame or a slit), fading in and out with the hand | `inventory.md`, `body-animator.md`, `icons.md` | the gestures within a frame (1.350 s against 1.342); the mask, the hinge and the icon: seen |
| Blind fire over the top and to the right (`BLIND FIRE`): its keys, refusals and ends, the body's states, the camera's tilt and the camera hung where the game's is, the overlap's line over cover; seen from outside from code | `blind-fire.md` | 62 of 63 commands' fates; the state, the body's two parameters and the shots' frames on every frame; the camera within 0.17 degrees, the eye to 0.1 mm, the weapon held within 1 cm; the weapon in the way in and in a change of side up to 5 cm off (open) |
| Repairing at the workbench (`REPAIR GEAR`, a kit let go on a thing): the game's repair window from its prefab, its figures by the client's code, the repair by SPT's server's | `workbench.md` | The AKM's figures against the digest's worked ones; the server's result by the rules alone; the window looked at, not set beside the game's |
| Durability, malfunctions and overheating (`DURABILITY`, `MALFUNCTIONS`, `OVERHEATING`): a shot's wear and heat, the chance and the roll, the five kinds in the hands (held, looked at, repaired), the keys' and the inventory's refusals, the stray shot and the rate by the heat, the spread by the heat and the worn barrel; the game's notices and glow, the inventory's red, the barrel's glow | `malfunctions.md` | `courses2\5-malfunction`: the AKM's chances, wear and heat at a shot to five places; its jam's look and repair within a frame; the rest read from code, the screens looked at |
| The left shoulder (`LEFT SHOULDER`, `Mouse4`): the key and its refusals, the game's controller, what takes it off and puts it back (the sprint, the jump, lying down, a vault, a reload, the inventory, a wall, a mount), the body's own move, the weapon put by the body's weapon bone, aimed before the eye, the camera after it | `left-shoulder.md` | Course 8's run with the MDR: the stance's flag and curve on every frame, the weapon root within 4 mm and 1.4 degrees, the camera within 2 mm; the wall, the jump, the vault and the mount read from code |
| Weapon mounting and the bipod (`MOUNTING`): the search for cover ahead or beside, the mount, its limits, stance and lean by the look, its exits, the weapon and camera on it, the bipod's toggle and gesture, the sounds, the stance panel's mount icon; seen from outside from code | `mounting.md` | Course 8's search answers and the bipod's gesture and operation on the game's frames; against the M700's mounts by hand the data, the approach, the stance by the pitch and the lean by the yaw to 0.0001, the exits and the sounds to the frame; the M4A1 on a window sill (`courses7`) likewise, and the pivot turned by the frame before's turn as recorded (10.3 mm a degree against 10.7); course 8 held no mount; side and bipod mounts read from code |
| Tactical devices: weapon lights and lasers, the head light, their keys in Press and Hold, the hands' switch and its sound, the hip penalty by ergonomics | `tactical-devices.md` | 58 presses' states as recorded; the hands' switch and its sound on the game's frame at 180 moments; the head light's waits on the game's frames, its light within 1.5 mm; the hip penalty to the last figure; the lights and lasers: seen |
| Doors, containers, a trunk, kicks, a lock and its key, the left hand's gestures | `interactions.md` | thirty recordings by hand: angles to 0.000 degrees, gestures within a frame |
| The keys as the game's input manager hears them; the options tab | `settings.md` | the keys on the game's own frame; the options: nothing |
| The head's free look (middle mouse) | `free-look.md` | a recording by hand: the head's turn, the mouse's hold and the view's turn on every frame |
| The sights as the game picks them: scopes, red dots, backups (the next sight, a scope's modes, a variable scope's zoom and its ring, 35 degrees through an optic, a canted sight's turn of the weapon); with `PIP`, the scope's own picture in its lens (its reticle, its shadow, its fringing, the flash's and the hits' effects) and the red dots' dots; without it, the world zoomed by the scope's magnification, the weapon at its size, the lens clear with its reticle and the game's vignette once the aim settles, black before; zeroing (the keys, the optics and red dots turned to the bullet's path, the M32A1's ranging) and the HUD's distance and magnification | `optics.md` | a recording by hand: both fields of view, the zoom, the sight's index and mode, the zeroing distance on every frame (within 0.00001 degrees); the zeroing's distance, sight and squeeze on every frame of three weapons' scripted recording, the optics' turns within 0.0002 rad; a canted sight's turn on every frame of three scripted recordings (within 0.007 degrees); the picture: seen |
| The character as others see it (`F5`) | `third-person.md` | twelve recordings from outside: the states on every frame, the arms within 0.1 to 1.4 degrees, the reach for the vest within 0.013 |
| Empty hands | `inventory.md` | appearing: by hand; the rest: nothing |
| A weapon of one's own (the StG 44, on the FN SCAR-L's animations); the catalogue of donors | `custom-weapons.md` | its donor, frame for frame |
| **Outfits from another game**: S.T.A.L.K.E.R.'s Sunrise and SEVA suits as the body, with that game's own first-person arms | `outfits.md` | nothing (neither game shows it); seen beside Tarkov's hands |
| **Health**: seven parts, bleeding, fractures, pain, hunger and thirst, falls, barbed wire; medicine, food and drink used with their own hands; the stimulators' and buffed foods' buffs (health, skills, stamina, the weight's limits, the damage taken, their side effects); the profile's voice (hurt, death, agony, breathing, muffled behind a face shield) and a bone's crack; exhaustion's and a painkiller's desaturation, tunnel vision; death, and `Enter` to begin again | `health.md` | fifteen recordings to the frame and to 0.01 point; the items' twelve uses on the game's frames; the desaturation's figures on every frame of two recordings but one (the buffs and the voice: from the code; the voice's clips from the install) |
| **The camera's sharpening** (`CAMERA SHARPENING`, with `CUSTOMS LIGHT`): Customs' camera's `CC_Sharpen` at the player's `Sharpen`, the weather's fade toward grey, the health's desaturation and a painkiller's ring, by the game's compiled shader | `health.md`, "Desaturation and tunnel vision" | the figures as recorded; the sums from the shader; captures |
| **The game's own physics engine**, PhysX 4.1 set up as the game's: the corpse, things dropped and thrown | `physics.md` | the corpse within 1.5 to 3.5 cm of four recorded deaths; things thrown: nothing |
| **`TARKOV UI`**, a switch: the game's own screens, panels, texts and sounds, read from the install; the inspect window with a thing's figures and its model, turned and zoomed; a container's window; the interaction panel, its hand and the loot sense's dot; the mouse's pointer | `inventory.md`, `inspect-window.md`, `interactions.md`, `sound.md` | GEAR and HEALTH beside the user's captures; medicine's rows beside four captures (Grizzly, CALOK-B, SJ1, Propital), row for row; the interaction panel beside the user's four pictures, to the pixel; the sounds and texts beside a recording out of a raid |
| **The sounds of moving**: footsteps by the surface underfoot (the game's twenty), sprint, jump, landing, crawl, turn, kit; a sampler of every surface (`--start surfaces`); a vault's and a climb's four marks (the jump-off, the grab, the slide, the landing) from the surfaces' vault sets with the kit, at a volume by the obstacle's height; a dummy's steps and kit heard from outside, as the game plays another character's (its group's own falloff curve) | `footsteps.md` | the vault's: the game's own vault recording, every clip, gap and level; the rest nothing of the game's |
| **The things' sounds** (`WORLD SOUNDS`): a door's, a crate's, a case's, a box's, a drawer's and a corpse's opening, squeak and shutting, a kick's breach and thud, from the clips their Customs prefabs name, at Tarkov's volumes and on its frames, plain stereo within 3 m; a weapon landing (thrown, or let go by a death) by its surface, kind and the fall's energy; a thing taken up off the ground | `interactions.md`, "A thing's sounds"; `sound.md`, "A weapon landing" | the doors' clips, volumes and rolloffs against the game's recordings; the containers, the landings and the pickup from the code and the install |
| **The hands' sounds**: what a weapon, an item used and the empty hands sound as their animation marks it, from each one's own sound player; the sights brought up (the kit, the weapon's aiming clip); Vorbis clips read with FMOD's headers from the game's engine | `sound.md` | nothing of the game's; every Vorbis clip decodes to its own length |
| **The shots' sounds**: each weapon's own banks, an automatic weapon's fire as the game's recorded loop ended on the next beat by its tail, any other's each shot whole; the game's own audio mixer read (its master compressor on the mix) | `sound.md` | nothing (no recording of a shot with sound) |
| **Grenades and the knife** by their keys: drawn, the pin pulled, thrown from above or below on the game's physics engine and gone at the fuse, the blast hurting, heard, felt and seen (its fireball, smoke, dust, concrete chips bouncing off the range, the flash's light, the shock wave and heat shimmer, the scorch left on the ground, a flashbang's sparks, smoke and light, each by the game's own shaders); the knife's slash, stab and quick stroke with their stamina | `grenades-and-knife.md` | the hands within a step of course 2's recorded timings; the throw's direction and speed as recorded |
| The game's texture quality, mip streaming and shadows | `streaming.md`, `settings.md` | nothing; 48 rifles: 1,970 MB of pictures on the card without streaming, 232 MB with; a raised picture waits for the card (0.140.1): no drawn thing left without its material in 13 walks, up to 12 a frame before |
| **The OVERALL tab and the clothing**: the model by the game's own camera, turned by a drag; every side's suits in its two dropdowns, each read when first chosen and put on the body, the models, the corpse and the hands | `inventory.md` | nothing (no recording of the page) |
| **The range lit as Customs on a morning, through Tarkov's camera** (`CUSTOMS LIGHT`): the sun, the sky and the ambient worked out by the game's own time of day from the install, the sky drawn by its own shader; smoke, dust and fire lit by the same; the camera's exposure, vignette and filmic curve (PRISM), its own colour filter and the sky's scattering over the scene; a range hall lit as the game lights an indoor place (its exposure, the sky's light taken away inside, the sun through its windows), with Factory's ceiling lamps (`LEVEL LAMPS`); any hour by the options' `TIME OF DAY` (with `CUSTOMS LIGHT` off, the range's own light carried through Customs' day and night at once) | `lighting.md` | the light's figures and the camera's settings as the game ran them on Customs at 09:37 (Factory's before), to the fourth place or better; the picture: nothing |
| **The game's ambient occlusion** (`AMBIENT OCCLUSION`, at the GRAPHICS page's `SSAO`, the install's own): its camera's HBAO by the game's own passes, after the opaque scene | `ambient-occlusion.md` | the preset as the game ran it at the user's setting; the picture: nothing |
| **Customs' clouds and their shadows** (`CUSTOMS CLOUDS`, at the GRAPHICS page's `CLOUDS QUALITY`): the weather's cloud layer by the game's own three passes, drifting with the wind, its shadows on the ground through the sun's cookie; the moon in the sky | `lighting.md` | the figures read and the light's axes against the recorded morning; the picture: nothing |
| **Ballistics** (`BALLISTICS`): each shot's bullets from the fireport, spread by the weapon and the round, flown as the game flies them (its G1 drag, its 0.01 s table) and meeting the range's boxes, doors and ground by Tarkov's materials: through, turned, in pieces, glancing off or stopped by the game's rules and its own random tables (Unity's generator, read from the engine); each hit's hole, dust, chips, sparks, light and sound by its material, a ricochet's whine; a character's hit colliders, its armour and the damage that reaches its health (`body-hits.md`), and two PMC dummies down the lanes to shoot, each holding an AKM, one standing in its idle and one walking side to side (`TARGET DUMMIES`, `F9` dresses them as you and stands them up) | `ballistics.md`, `body-hits.md` | 278 recorded bullets flown again within millimetres; 528 of 545 recorded hits off bodies with the game's outcome; the recorded hits on Tagilla and a Scav (44 bullet hits, 44 delivered) to the last figure; the hits' show and sound: nothing |
| **The handheld rangefinder**: the Vortex Ranger 1500 from a quick key, its hands, aim and optic, its readout every 0.5 s to 999 m drawn red in the optic, its aim's stamina | `rangefinder.md` | nothing recorded: read from the code, tested and seen |
| **Reactive targets** (`REACTIVE TARGETS`): the hideout range's steel poppers down the lanes, 25 to 300 m, folded by a hit on the face and stood up 3 s later, rocked by a hit on the neck, with their sounds; a distance sign at each row | `range-targets.md` | nothing (no recording of the hideout's range): tested and seen |
| **Tarkov's surface shading** (`TARKOV SHADING`): every surface lit by the game's own deferred light and ambient pass, in its gamma-space numbers; the sun, the sky's light, the hall's lamps and an explosion's flash alike | `lighting.md` | the rule from the game's shaders and the engine; the picture: nothing |

| **The workbench and the edit build screen** (`WORKBENCH`, `ATTACHMENTS`): the main metal table of the hideout's own level 3 workbench read from the install, and at it the game's edit build screen for the gun in hand, drawn as the game's own with `TARKOV UI`: turned by a drag, every slot by class on the game's ellipse, every part of the game that fits a slot, ASSEMBLE; weapons drawn and handled by their own parts | `workbench.md` | nothing (no recording of the screen) |
| **Skills** (`SKILLS`): every skill's level set on the options' SKILLS page (0 to 51) or by a host game in play, never earned; each buff, by the game's own rule, wherever what it acts on is built: stamina and the breath, the top speeds, the jump and the weight's limits, health's odds and drains, the steps' noise, a weapon's reload, draw, ergonomics and recoil, the aiming speed, magazines; the inventory's SKILLS tab as the game's page, those that change nothing here dimmed | `skills.md` | nothing; every figure from the code (the regression the same to the byte at level 0) |

575 tests pass. The range can be edited in Blender and read back (`docs/manual.md`, "The range in Blender"). The regression over the sixteen kept recordings is run after every change to the simulation.

**Not built** (in full: `docs/status.md`, "Missing"): earning skills (the levels are set); the world's sounds but the things'; the muzzle's light; malfunctions, bodies hit (and their blood), zeroing; the knife's hits; a rocket's flight; slopes and sliding; searching item by item; a second character; Tarkov's shading's second part (the surfaces' own figures) and the camera's other effects (its blooms, colour lookup, sharpening); the STALKER host.

**Placeholders.** Every guess in the code is marked `ESTIMATE` and every unconfirmed rule `PROVISIONAL`; all are listed in `docs/status.md`, "Placeholders". Those that show most: the test range and its crates, which are the app's own; the StG 44's round, recoil and size.

## Requirements

- Windows, Rust (stable, MSVC toolchain) with the Visual Studio C++ build tools: `Install.bat` offers to install both.
- Single Player Tushonka (SPT) 4.1, installed over your own copy of the game (its client 0.16.9). SPT and the mods for it are on its Forge (`sp-mod.com/installer`). The item database is read from SPT's files, so the game alone is not enough.
- A GPU that supports BC texture compression (any desktop GPU).
- Optional, for the suits from S.T.A.L.K.E.R.: Anomaly, alone or under GAMMA. Both are read, never written; without them the suits are items with nothing drawn for them.

## Running

**`Install.bat`**, at the repository's top, once: it checks for the C++ build tools and Rust and offers to install what is missing (through winget, asking first), builds the app in release, and asks where your games are. **`Play.bat`** then starts it (`cargo run --release -p trp-app`, rebuilt first only when the code changed), passing on anything after it (`Play.bat --equip`). By hand, from this folder:

```
cargo run --release -p trp-app
```

**The first start asks where your games are** (`Install.bat` asks it; a run that finds no install asks it in its console): your SPT folder, then, if you have them, GAMMA's folder and Anomaly's, each asked for itself and either skipped by Enter. Nothing is searched for: each question offers only what you gave it before. The answers are checked and kept in `local/` (`eft_dir.txt`, `gamma_dir.txt`, `anomaly_dir.txt`); `--setup` asks again, and `EFT_DIR`, `GAMMA_DIR` and `ANOMALY_DIR` do instead. The first line printed is the version (`--version` prints it alone; `docs/manual.md`, "Versions").

The first build compiles Bevy and PhysX 4.1 from its source: in release, about four minutes here and a build folder of 2.2 GB (2.4 GB with the tests) (the workspace in debug, `cargo build --workspace`, 4.1 GB, 5.1 GB with the tests). With Rust (1.5 GB), the C++ Build Tools and Windows SDK (5.1 GB) and the crates downloaded (about 1 GB), about 10 GB from nothing, besides SPT; the repository itself is 13 MB. The character starts bare on a test range; **the crate straight ahead holds a rifle, a vest and a backpack, and the tables along the low wall hold every weapon of the install** by class (look at one and press `F`; Alt and a click puts a thing on; `Tab` shuts the screen; `2`, `3` or `1` takes the weapon in hand).

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
| `Left Alt` + `W` / `S` (hold) | Blind fire over the top / out to the right |
| `V`, `Left Ctrl` + `V` | Rest the weapon on cover (again to leave); put a bipod out or fold it |
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

The unit tests need nothing; those in each crate's `tests/` folder (one binary a crate: `--test sim`, `--test physics`; a file's tests by its name, `--test sim install::`) run against the configured install and pass with a note when there is none (`--nocapture` prints what they measured); a few need the recordings in `local/traces` too.

## Layout

```
crates/trp-data    readers for the install (Unity bundles, the SPT database, animators, the settings
                   object, the screens), binary FBX, S.T.A.L.K.E.R.'s models; bin/probe
crates/trp-sim     the controller as a plain simulation with no engine in it; bin/replay, bin/byhand
crates/trp-physics the game's physics engine (PhysX 4.1) set up as the game's; bin/corpse
crates/trp-app     the Bevy application: the range, the rigs, the screens
hosts/spt-oracle   the recording plugin for SPT (C#) and its scenarios (private repository only)
custom/            the weapons and outfits that are not the install's (their models outside the repo)
docs/              spec/ (how the game behaves), status, manual, backlog, findings; decisions,
                   workbench and history (private repository only)
local/             machine-specific settings, tools, traces and scratch output, never committed
```

## Reading this repository

**Who speaks.** "The user" in the documents is the project's maintainer, whose decisions are quoted with their date; the rest is the account of the AI sessions the work was done in, written as each thing was found. A date is the day of the session that wrote the line.

**The words the documents use.** *The install*: your own SPT install, read as the app runs, never written. *The game's* is Tarkov's own behaviour or value, *the install's* a value read from your SPT files, *the app's* TRP's own choice. *The figures* are numbers (constants, settings, measurements); *the picture* is what is drawn. *A thing* is an inventory item. *The hands* are the game's hands controller and its animations, *the arms* the first-person arm meshes. *The rig* is a weapon's or item's assembled model with its bones, built from the install's *prefab*. *The engine* is Unity's native player in the install, read with Ghidra where the C# does not say. *The recorder* (`spt-oracle`, private): the project's own plugin for SPT, which recorded what the game's character, camera and weapon did, frame by frame; *a trace* is one such file of frames, *a recording* a run of them, *scripted* (under the recorder's own input) or *by hand* (the maintainer playing); *the courses*: the recorder's numbered lists of recordings still wanted, one numpad key each; `courses1`, `courses2`, `courses3` are the folders of the maintainer's runs of them. *Compared with the game*, *set beside*, *frame for frame*: the simulation run again under a recording's input and set beside it value by value (a weapon of one's own is set beside its *donor*, the install's weapon it borrows its animations from); where nothing of the game's could be set beside a part, the documents say *nothing* or *from the code and the install alone*. In the specs the bold verdicts say how far a piece was checked: *Read* from the game's code, *Built*, *Seen* in a capture of the app, *Tested* by a cargo test, *Compared* with a recording, *Not built*. *The regression*: the kept scripted recordings replayed through the simulation after a change and their reports compared with the last kept run's (`after130`). *The probe*, *the replay*, *byhand*, *bullets*: tools in the crates' `src/bin` (`docs/manual.md`). *A step lands*: a piece of work is finished, the version raised and the step written up; *the account* (`docs/status.md`) lists each step and what it was compared with; *the placeholders* (`docs/status.md`) list every `ESTIMATE` and `PROVISIONAL` in the code. *The range*: the app's own test level, with its crates and tables of weapons; it is not the game's. *The dummy*: a character stood on the range to be shot. *A switch*: in the options' features, the switch of a built system (restart required; off, the system does nothing); `TARKOV UI` draws the screens as the game's, off as the app's own, with the same figures either way. *The passthrough*: TRP run under a host game (STALKER GAMMA first), the host feeding input and collision and drawing the poses it gets back. *The spec*: `docs/spec/`, how the game behaves, in our own words.

**What can be verified here, and what cannot.** The tests run against your own install. The recordings, the recorder and its scenarios are not published, so the comparisons with the game cannot be run again from this copy: a change to the simulation can be built and tried, not measured against the game the way the documents' figures were. Say so when you report one. A pointer to `history.md` (the log), `workbench.md` (the scratch tools and the recordings one by one), `decisions.md`, `hosts/spt-oracle` (the recorder) or `.claude/` (the maintainer's agent skills, hooks and checks, among them the version's `check.sh` and the placeholders' `placeholders.sh`) names the private repository; `local/` is each machine's own and never committed.

**How to read.** The documents are long (`findings.md` 460 KB, `status.md` 220 KB, a spec up to 215 KB), each line a paragraph: list a document's headings (the lines that begin with `#`) and read the one section, rather than the file. Some source files over 2,000 lines have `// ==== ... ====` lines for their sections: list them first. `findings.md`'s dated sections are newest last.

**For a coding agent.** `AGENTS.md` at the repository's top is the brief an agent (Claude Code through `CLAUDE.md`; Codex, Cursor and Copilot on their own) loads before it starts: the rules any change must keep, and where things are.

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
- **File formats.** Unity's from the files themselves (bundles describe their own layouts) and the public documentation of open tools (AssetRipper, AssetStudio, UnityPy). S.T.A.L.K.E.R.'s models and packed archives from that engine's published source (OpenXRay), the archives' compression written afresh from the published LZHUF and LZO algorithms. Binary FBX as Blender's developers have written it up. FMOD's sound banks (FSB5) as the open vgmstream project documents them; a Vorbis clip's setup header, which FMOD leaves out of the bank, is read from your own copy of the game's engine each time, and the clip decoded by `lewton`. A crunched picture (Unity's `DXT1Crunched`) is decoded by `texture2ddecoder`, a published port of Unity's own crunch.
- **The screen's effects.** What the game's camera does to its picture (a contusion's, a flash's, a hit's, pain's) was read from the compiled shaders in your install's own shader bundle, disassembled on the author's machine with the Windows SDK's `fxc /dumpbin`, and written down as sums in `docs/spec/`; the shaders here were written from those notes.

**Not affiliated** with or endorsed by Battlestate Games, GSC Game World, or the teams of Single Player Tushonka (SPT), Anomaly or GAMMA. Escape from Tarkov and S.T.A.L.K.E.R. are trademarks of their owners, named here only to say which games this works with.

**License**: Apache-2.0 (`LICENSE`, at the repository's top), for this project's own code and documents alike (the maintainer, 2026-10-22): copy either with the notice. It grants nothing in the games it reads.
