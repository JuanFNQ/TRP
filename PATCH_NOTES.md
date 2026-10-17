# Patch notes

## TRP 0.42.2: the installer waits for you (2026-10-17)

- `Install.bat` now says what it will do and what you need, then waits for a key before it installs or builds anything. To stop, close the window.

## TRP 0.42.1: the installer asks for each folder, it doesn't search (2026-10-17)

- **Asked, not searched for**: `Install.bat` and the first start no longer look through your drives for SPT, GAMMA or Anomaly. Each question offers only the folder you gave before, so press Enter to keep it, or paste another.
- **One question per folder**: SPT (needed), then GAMMA and Anomaly (both optional). Anomaly's folder is asked for even if GAMMA's Mod Organizer names one.

## TRP 0.42.0: health, Tarkov's own interface, sound, and a two-click install (2026-10-17)

The first public release since 0.9.1 (2026-10-14): forty-nine versions of work at once, so this one is long. TRP reads SPT 4.1.6 (game client 0.16.9.5), SPT's last version, so nothing it reads moves under it.

In these notes, **compared with the game** means checked frame by frame against recordings of the running game; **from the game's code** means written from reading the game, not yet checked against a recording.

### Highlights

- **`Install.bat` and `Play.bat`**: double-click to install, double-click to play. No more cargo commands to type.
- **Health and death**: seven body parts, bleeding, fractures, pain, hunger and thirst, falls, barbed wire; medicine, food and drink used with their own animations; death as a ragdoll on Tarkov's own physics engine.
- **TARKOV UI**: an option that draws the inventory and the in-raid HUD as Tarkov's own, read from your install: slots, item views, grids, the health panel, the quick bar, the inspect window, the right-click menu, the fonts.
- **Sound**: interface and item sounds, footsteps by surface, weapon and item handling, gunshots and aiming, through the game's own mixer settings.
- **Rendering**: Bevy 0.18; Vulkan by default, DirectX 12 as an option; Tarkov's texture quality, mip streaming and shadow presets.

### Install and launch

- **`Install.bat`** (run once) checks for the Visual Studio C++ build tools and Rust, and offers to install whichever is missing (through winget, after asking). It then builds TRP and asks where your games are. The winget installs have not yet been tried on a machine without the tools. If one fails or you say no, `Install.bat` opens the download page so you can install it by hand.
- **`Play.bat`** starts TRP. It rebuilds only when the code has changed, as after an update. Anything typed after it is passed on: `Play.bat --equip`.
- **Fewer first-start questions**: your SPT folder is found by itself when it is in a usual place (`C:\SPT`, `D:\Games\SPT` and the like), and GAMMA and Anomaly are now one optional question. Give either folder; GAMMA's Anomaly is found through its Mod Organizer.
- **A release build**: about 10 GB on disk from nothing (Rust, the C++ tools, the downloads and the build), down from about 12 GB for the debug build.
- **A smaller repository**: the public copy holds only what building and running needs. The research plugin, its scenarios, the recordings and the working notes stay private.

### Health and death

- **Seven body parts**, each with its own health; bleeding, fractures, pain, hunger and thirst, falls by height, barbed wire. Each affects movement, stamina and weapon handling as the game's does.
- **Death**: the body becomes a ragdoll set up as the game's player prefab; the weapon drops on its sling; the view rides the chest while the colour fades and the eyes close; then the death screen. `Enter` restarts the run in place.
- **Medicine, food and drink**: bind one to `4` to `0` and press the key, double-click it in the inventory, or drag it onto a body part on the HEALTH tab. The item's own hands play its animation. The left or right mouse button cancels, and whatever was already healed stays healed. A window asks how much food or drink to use.
- **The HEALTH tab**: every part, with its effects as the game's icons and tooltips.
- **New on the range**: barbed wire, a 6 m ledge, and a MEDICAL table with every medical item, food and drink.
- **Testing keys**: `F7` hurts the next body part, `F8` kills.
- **Compared with the game**:
  - the medicine course's twelve uses, the falls and the barbed wire, to the frame and to 0.01 of a point;
  - the corpse within 1.5 to 3.5 cm of four recorded deaths.

### Tarkov's own physics engine

- A new crate, `trp-physics`: NVIDIA's PhysX 4.1, the version the game uses, built from NVIDIA's published source and set up as the game's Unity sets it up.
- Items you drop or put down are thrown and land on their own models and colliders, and the quick reload's magazine falls to the floor. Switch: `ITEM PHYSICS`.

### TARKOV UI (Options > FEATURES, off by default)

- **The GEAR tab as the game's**:
  - the equipment slots over the character's outline;
  - every item in the game's own item view (counts, durability, marks);
  - the containers' column with each container's own grid layout, scrolled as the game's;
  - the stats strip (weight, health, hydration, energy), the striped footer and the tab bar.
- **The HEALTH tab** as the game's panel: the silhouette, its colours, the effect icons.
- **The rest of the screens**: the container panel when looting, the quick-use panel, the inspect window on a double click, and the right-click menu.
- **In a raid**: the stance panel, the health silhouette and the quick bar. They fade in and out as your game's settings say (always, autohide or never: Options > GAME).
- **Drawn as the game draws it**:
  - the game's fonts from their own atlases, with TextMeshPro's shader, outlines and shadows;
  - the world blurred behind the inventory as the game's camera blurs it;
  - the screens composed in gamma space as Unity's are.
- **0.41.2 drawing fixes**: no more one-pixel gaps or light seams between pieces. Slot headers, the quick-use bar, the menu's lines and the inspect window's shadow are whole, and text edges are no longer cut off.
- **Compared**: the GEAR and HEALTH tabs, side by side with captures of the game at 1080p and 1440p. **Not compared yet**: the HUD in play.
- **With the switch off**, the app keeps its own look, with the same figures.

### Inventory

- A right-click menu on any item, with or without TARKOV UI, showing the lines a raid shows.
- A double click uses medicine, food or drink.
- Every run has its own pockets and a dogtag, as a game character has.
- The RPK-16 now comes with a magazine. Its stock build has none, so the app gives it the one its template names.

### Sound

- **Interface and item sounds**: opening and closing the inventory, tabs, picking up, putting down and fitting items, windows. They play at your game's volumes, or at those set on the new Options > SOUND page.
- **Footsteps** by the surface underfoot, on all twenty of the game's surfaces, plus sprint, stop, jump, landing, crawl, turning and kit rattle, by the game's rules. A sampler of all twenty surfaces stands on the range (`--start surfaces`). Switch: `MOVEMENT SOUNDS`.
- **The hands**: each weapon's handling sounds and each item's use sounds, played on their animation's marks by their own sound player. Switch: `HANDS SOUNDS`.
- **Gunshots**: each weapon's own sounds. Automatic fire plays as the game's recorded loop and ends on the beat with its tail; single shots play whole. Switch: `SHOT SOUNDS`.
- **Aiming**: the kit rattles when the sights go up or down, and the weapon's aiming sound plays, louder for a less ergonomic weapon.
- **Mixed as the game's**: the game's audio mixer is read from your install. Its master compressor is applied to the mix, and footsteps sit at the game's -3.5 dB.
- **Vorbis clips** are decoded too, their setup headers read from your own copy of the game's engine.
- **Not compared with the game yet**: there is no recording with sound so far, so the levels come from the game's files.

### Rendering and performance

- **Bevy 0.18** (wgpu 27).
- **Vulkan by default**: Options > GRAPHICS > RENDERER switches to DirectX 12 (restart required). If the chosen one finds no graphics card, TRP falls back to the other.
- **TEXTURE QUALITY** (low, medium, high) and **MIP STREAMING**, as the game's. With streaming on, 48 rifles take 232 MB of textures on the graphics card instead of 1,970 MB.
- **SHADOW QUALITY**: the game's own shadow presets (at medium, two cascades out to 40 m).
- **Up close**: nothing nearer than 10 cm to the camera is clipped away any more (a weapon aimed close up), and the first-person body casts its whole shadow.
- **GROUNDED FEET** (option): the body is drawn down onto the ground, where Tarkov's collision capsule rests slightly above it.
- **2 to 6% shorter frames**: hidden interface pieces are left out of the layout.
- **A smaller build folder**: about 4 GB from nothing, with dependencies built without debug information.
- **Fixed**:
  - crashes from DirectX 12's sampler limit with many items on the ground;
  - a blur and blue flashes after closing the inventory;
  - a crash when a sprint lands after a fall.

### Options

- **FEATURES**: a switch for every finished system, in GAMEPLAY and VISUALS sections. Hover a row to see what it does. Each takes effect at the next start.
- **New pages**: GAME (when the HUD shows, the health colours) and SOUND (the overall and interface volumes).
- **`F1`** shows one panel with the frame rate, the debug figures and the key guide. It replaces `F3`.

### Checked against the game since 0.9.1

- **Now compared frame by frame** against recordings made by hand: the Five-seveN, Model 700, SV-98, SKS, Rhino and MP-43 (the MP-43 and Five-seveN up to a weapon switch).
- **Also compared**:
  - the AKM's quick reload, its look-over and its other keys;
  - doors, kicks, a lock and its key, and searching;
  - the free look (the head's turn on every frame);
  - the character as others see it (`F5`), with the arms within 0.1 to 1.4 degrees.
- **Fixed by those comparisons**:
  - a cocked revolver's shot timing;
  - the weapon's actions timed on the game's millisecond clock;
  - a sprint into barbed wire, which now ends when a body part, not the capsule, reaches it;
  - working a door or container, timed by the player's clock.

### Not built yet

- Grenades and the knife on their keys.
- Malfunctions, ballistics, optics, mounting and blind fire.
- Slopes and sliding.
- A second character.
- Skills as a system.
- The world's sounds.
- Tarkov's own shading.
- The S.T.A.L.K.E.R. host.

The full list is in `TRP/docs/status.md`. With TARKOV UI on, three small differences remain: item captions sit 2 to 3 px right of the game's, the loot panel's `LOOT` title sits a little high, and the panels' cells are slightly darker.

### Requirements

- Windows 10 or 11 and a desktop graphics card.
- Your own copy of Escape from Tarkov with SPT 4.1 installed over it (client 0.16.9.5, SPT 4.1.6).
- Optional: S.T.A.L.K.E.R. Anomaly, alone or under GAMMA, for its suits.
- The C++ build tools and Rust: `Install.bat` installs them if you say yes.

## TRP 0.9.1 (2026-10-14)

Every weapon of the install on the range in labelled crates by class, the character model on the gear page, the inventory's pictures drawn by a copy of the game's icon camera, and a performance pass.

## TRP 0.1.0 (2026-10-14)

The first public version.
