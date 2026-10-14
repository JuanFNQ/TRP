# Settings: the keys, the display, and the options tab

How the game hears its keys and lets them be changed, which of its display settings the app takes, and the tab the app keeps them on. Read from the game's code on 2026-10-12 (`EFT.InputSystem`, `EFT.Settings.Control`, `EFT.Settings.Graphics`, `EFT.UI.Settings`, `MoveInputTranslator`, `FirearmHandsInputTranslator`, `PlayerInputTranslator`, `Player`), from the install's own settings (`SPT_Runtime\user\sptSettings\Control.ini`, `Graphics.ini`, `Game.ini`, each a page of JSON), from the build's input settings (`EscapeFromTarkov_Data\globalgamemanagers`) and from the scene that holds the game's settings screen (`level44`). **The keys' timing has been set beside seventeen recordings made by hand** (below, "Compared with what"): the five bindings whose doing shows in a recording all come on the game's own frame. The settings screen itself, and everything of the mouse, is from the code and the install alone. What is built, and what was run, is at the end.

## Where the game keeps them, and where the app does

The game's settings are a screen of its menu (`SettingsScreen`) with five tabs: Game, Graphics, PostFX, Sound, Controls. **The user asked for them inside the inventory screen, as a tab at its far right (2026-10-12).** That is the app's arrangement, by the user's word; what the tab holds is the game's wherever the game has it. It has three parts:

- **Controls**: the game's bindings, changed by the game's rules (below).
- **Graphics**: the four of the game's settings the user named: VSync, the field of view, the resolution, and how the window is shown.
- **Features**: a list of the systems a host might switch on and off. **A list only: nothing of it is switched** (the user, 2026-10-12: "nothing coded under this sub-tab just yet"). **Over the list, what is switched: three things**, each by the user's word and each taking effect when the app is next started ("Features", below).

The install is never written to. What is changed is kept in the app's own file, `local\options.json`; without it the app starts from the install's bindings, as before.

## A binding

A binding (`KeyGroup`) is:

- the game's name for what it does (`EGameKey`: `Shoot`, `ReloadWeapon`, `LeanLockLeft`);
- up to two **variants** (`InputSource`), each a set of keys held together, which may end in a notch of the mouse wheel one way (`Mouse Scroll`, the only axis a binding may take);
- **how it is pressed** (`EPressType`, in this order): `Release`, `Continuous`, `Press`, `DoubleClick`.

Seven **axes** are bound beside them (`AxisGroup`: `MoveX`, `MoveY`, `TurnX`, `TurnY`, `LookX`, `LookY`, `LeanX`), each with two pairs of a positive and a negative variant. The mouse's four are not shown to be changed; the keys push the other three.

As this install has them (the ones the app acts on; `probe keys <part of a name>` prints any):

| Binding | Keys | Pressed |
|---|---|---|
| `MoveX`, `MoveY` | D against A, W against S | axes |
| `LeanX` (the slow lean) | D with Left Alt against A with Left Alt | axis |
| `LeanLockLeft`, `LeanLockRight` | Q, E | `Continuous` |
| `StepLeft`, `StepRight` | Q, E with Left Alt | `Continuous` |
| `Sprint` | Left Shift | **`Press`** |
| `Walk` | Caps Lock | `Press` |
| `Jump` | Space | `Release` |
| `Vaulting` | Space | `Continuous` (the only way it has) |
| `Duck` | C | `Release` |
| `Prone` | X | `Press` |
| `Next`, `Previous` (the speed) | the wheel up, down | `Press` |
| `NextWalkPose`, `PreviousWalkPose` (the stance) | **C with the wheel** up, down | `Press` |
| `Shoot` | left mouse | `Continuous` (not the player's to choose) |
| `Aim` | right mouse | **`Press`** |
| `Breath` | Left Alt | `Press` |
| `ReloadWeapon` | R | `Release` |
| `QuickReloadWeapon` | R | `DoubleClick` |
| `ShootingMode` | B | `Press` |
| `ForceAutoWeaponMode` | B | `DoubleClick` |
| `CheckFireMode` | B with Left Alt | `Press` |
| `ExamineWeapon` | L | `Press` |
| `DropBackpack` | Z | `DoubleClick` |
| `CheckAmmo` | T with Left Alt | `Press` |
| `CheckChamber` | T with Left Shift | `Press` |
| `FoldStock` | L with Left Alt | `Press` |
| the weapon slots, the inventory, `Interact`, `ThrowItem`, the quick-use keys | `inventory.md`, "The keys" | |

So in this install **the sprint and the aim are each one press on and one press off, not held**, and the stance is the crouch key with the wheel. Where an install has no `Control.ini`, the app takes these as read.

## How a key becomes a command (`InputManager`, `InputPreset`, `InputKeyCombination`)

The manager makes one **combination** for every variant that has keys, and runs them all once a frame.

**A key's own state** on a frame is none, going down, held or coming up, from whether it is down and what it was. A notch of the wheel is a key that is down for the frame it turns in. Several keys together are none if any is none, else coming up if any is, else going down if any is, else held.

**The order.** Combinations with more keys run first; among those with as many, the double clicks first. A combination that acts **takes** its keys, for as long as they are held or for the one frame. A combination finding one of its keys taken gives way if the taker:

- is marked as never giving way itself (the aim's while it is bound `Continuous`; the tactical device's release);
- else, has the very same keys and is a double click (so a plain press waits for the double click to fail);
- else, where both have a rank of their own, has the lower rank (`CheckFireMode` 10, `FoldStock` 11, `CheckAmmo` 12, `CheckChamber` 13, `ChamberUnload` 14, `UnloadMagazine` 15; blind fire's two 99);
- else, has as many keys or more.

The sprint's combination never gives way (rank -2). An axis keeps its keys from blind fire's combinations.

This is what makes `T` with `Left Alt` the look at the magazine and not also the tactical device's `T`, and `E` with `Left Alt` the step aside and not the lean.

**How each way of pressing is heard:**

- `Press`: the command comes on the frame the keys go down (all held, the last one going down).
- `Release`: the keys going down are noted, and the command comes on the frame they come up, if the combination still has them. If a combination of *other* keys took one of them meanwhile, the release is not heard at all.
- `Continuous`, for a binding that **holds** (`ToggleInputKeyCombination`: the trigger, the aim, the sprint, the breath, the walk, the vault, the crouch, the two leans, the two steps aside, blind fire, interacting and a few of the voice's): the command comes as the keys go down, and its second command, the taking back, as they come up.
- `Continuous`, for any other binding: the command comes once the keys have been held for the double click's time.
- `DoubleClick`: down, up, down and up again, all within the double click's time of the first press (`DoubleClickTimeout`, 0.3 s in the install, 0.3 to 1). If it does not come, the keys are given back *as just pressed*, so that a plain press on the same key is then heard.

**Consequences, all the game's:**

- A key that also has a double click bound makes its plain binding **wait out the double click's time**: `R` let go reloads 0.3 s after it went down, `B` changes the fire mode 0.3 s late, `1` draws the pistol 0.3 s late. A player rids a key of the wait by moving the double click elsewhere.
- **The crouch key used with the wheel does not crouch.** The crouch is on the key's release, and the stance is the same key with the wheel. The manager tells the two apart by all their keys while the crouch is bound `Release`, so the key coming up after a notch is not heard.
- Both ways of an axis held at once push it the positive way (D and A together walk right, W and S forward): the way with more keys is looked at first, the positive one where they have as many.
- With Left Alt held, D and A are the slow lean's and do not walk.

**The wheel** (`InputAxis.Update`; read from the build's input settings on 2026-10-12, night). A binding's notch is the engine axis `Mouse Scroll`, and the game takes it for pressed on a frame whose axis reads **exactly** one that way. The build has that axis as the mouse's third (the wheel) at one a count, so one notch in a frame reads one and is a press, and **two notches in the same frame read two and are none**: a wheel spun fast loses notches, in the game and here (`keys::notch`). It was `PROVISIONAL` before (any turn of the wheel in a frame counted as one notch).

## The mouse (`InputAxis.GetValue`, `MovementState.ClampRotation`, `MoveInputTranslator.TranslateAxes`; read 2026-10-12, night)

The view's turn for a count of the mouse is a product of five figures:

| Figure | Where it is | In the install read |
|---|---|---|
| The engine's own halving of a raw count on Windows | not in the install or the game's code: the engine's (a post by one who read its code, discussions.unity.com/t/812499) | 0.5 |
| The engine axis's own factor (`Mouse X`, `Mouse Y`) | the build's input settings (`probe keys`) | 0.05 |
| The binding's factor (`TurnX`, `TurnY`: `InputSource.sensitivity`) | `Control.ini` | 5 |
| The player's sensitivity (`MouseSensitivity`; with the sights up `MouseAimingSensitivity` times the weapon's own factor) | `Control.ini`, 0.1 to 5 | 0.7, 0.5 |
| The movement state's own factor and limit | the body's animator (`movement.md`, "Looking") | 1 standing |

So **a count is 0.125 degrees before the sensitivity**, which is the figure the community's converters list for the game (csdb.gg). The app had 0.1 as an `ESTIMATE` (the engine's default for an axis, with the binding's 5 left out): it turned a fifth slower than the game. `keys::degrees_a_count` reads the two figures the install has; the engine's halving is a constant, and is the one figure here that is neither read nor in the game's code.

`InvertedXAxis` and `InvertedYAxis` turn the sign of what the mouse gives, across and up and down, for the turn and for the head's free look alike. `OpticSensitivity` (1 to 5) is what a scope's zoom is adjusted by with the wheel (`ScopeZoomHandler`); nothing here has a scope.

## What the bindings do (`MoveInputTranslator`, `FirearmHandsInputTranslator`, `Player`)

Each binding gives a command, and a binding that holds a second one when it is taken back:

| Binding | Given | Taken back |
|---|---|---|
| `Sprint` | the sprint is switched to the other of on and off (`ToggleSprint`; standing still it does nothing) | the sprint is ended (`EnableSprint(false)`) |
| `LeanLockLeft`, `Right` | leans that way, or straightens if leaning that way already (`ToggleLean`) | straightens, if still leaning that way |
| `StepLeft`, `Right` | steps that way, or steps back if stepped that way | steps back, if stepped that way |
| `Duck` | between crouched and standing, or back to the stance last chosen (`TogglePose`) | a character lower than half way is put back to the stance last chosen, or upright if that was as low (`RestorePose`) |
| `Breath` | the breath is held, or a held one let go | a held breath is let go |
| `Vaulting` | the vault's try is switched over | the same: both do the one thing (`Player.Vaulting`) |
| `Shoot` | the trigger is turned over | the trigger is let go |
| `Aim` | the sights the other way (`ToggleAim`) | the sights down (`SetAim(false)`) |
| `Walk` | the speed put to none and what it was put aside, or taken up again (`ToggleMinSpeed`; `movement.md`, "The walk key") | taken up again (`RestoreSpeed`) |

A binding that holds, bound as a `Press`, gives only its first command: **that is what makes the install's sprint and aim one press on, one press off.** Bound `Continuous`, the same binding is held.

The rest give one command: the jump, lying down, the speed's and the stance's notches (one `DELTA_SPEED` or `DELTA_LEVEL` each, from the game's settings object), the reload, the fire mode, the fullest fire mode, the looks at the weapon (its magazine, its chamber, its selector, all over: `firing.md`, "Three more keys"), the stock's fold, the backpack's drop (`interactions.md`, "A backpack dropped").

**The axes are handed out after the frame's commands** (`InputNode.TranslateInput`): the head's look, the walk, the turn, and last the slow lean, which leans a tenth of a degree a frame (`Player.SlowLean`; `movement.md`, "The slow lean").

## The controls tab as the game lays it out (`ControlSettingsTab`; the scene `level44`, `Common UI/SettingsScreen/Control Settings`)

On the game's screen of 1920 by 1080 units: a row of two toggles (the keys, and the phrases' wheel) 150 down; under it a content area 1720 wide; in it **one box of the bindings**, 1720 by 689, with a line of four heads (30 high) over a list that scrolls (649 high, rows of 28): what the binding does on the left two fifths, its two keys a fifth each, and how it is pressed on the last fifth; and **under it the mouse's panel**, 1720 by 116: a row of four bars 300 wide, each under its name and with its figure in a box to its right (the mouse's sensitivity, its sensitivity aiming, the double click's time, the sensitivity of a scope's zoom), and under them a row of two switches (the inverted X axis, the inverted Y axis). The bars' limits are the settings' own: 0.1 to 5, 0.1 to 5, 0.3 to 1, 1 to 5; the first two are written to three places and the others to two.

The game's words for all of it are in the install's texts (`Mouse sensitivity`, `Mouse sensitivity (aiming)`, `Settings/DoubleClickTimeout`, `Settings/OpticSensitivity`, `Inverted X axis`, `Inverted Y axis`, `EPressType/Release` and its three fellows, `Exit`). Not read: the words over the four columns (the scene names them by a key that was not read: `ACTION`, `KEY`, `KEY`, `PRESS TYPE` are the app's).

## Changing a key (`CommandKeyPair.ListenForKey`, `ControlSettingsTab.KeyChosen`)

**What is shown.** Each axis's two ways, then every binding but the function keys, Escape, Enter and the bindings the game makes out of others (`PressSlot4` and its like, `HighThrow`, `LowThrow`, `PressThrowGrenade`, `ReleaseTactical`, `ChangePointOfView`). A row is the game's words for the binding (the install's texts have them by the binding's own name: `Shoot` is "Fire", `Duck` "Crouch"), its two variants, and how it is pressed.

**Choosing.** A click on a variant listens: "Press any key...". Every key that goes down is added, one a frame, kept in the order of the engine's numbers for them. The choice is made when one of them comes up, or a third goes down, or the wheel turns a notch (not for the trigger). Escape among the keys drops it, and what stood there stays. There is no way to leave a variant empty but to give its keys to something else.

**The same keys on two bindings.** They may stay on both only if both bindings' way of being pressed is the player's to choose and the two are pressed differently (`R` let go and `R` double clicked). Otherwise:

- if either's way is not the player's to choose, or it has a notch of the wheel, **the other loses the keys at once**;
- if both may be chosen and they are pressed alike, **the game asks how the new one is to be pressed** (its list opens, if the binding's other variant is empty); answered the same way, the other loses the keys; answered differently, both keep them. It asks likewise when a binding that had no keys is given some, and sets it to `Release` meanwhile.

The same keys on an axis are dropped there, and keys chosen for an axis are dropped wherever else they stand. The same keys in the binding's own other variant are dropped there.

**How it is pressed** is chosen from the four, where the game allows it. It does not for: the trigger, the head's free look, push to talk, interact, blind fire's two, the screenshot, discard, the voice's switch, the left stance, the tactical device, the grenade. Three have one way only: the vault `Continuous`, the tactical device and the grenade `Press`. A binding with a notch of the wheel is a `Press`. Changing it drops the same keys from any binding now pressed alike.

A quirk kept: two sets of the same keys count as the same only in the same order. The install's file has some of its own in another order than a chosen set is kept in (`Mouse1` before `LeftControl`), so those two would not be seen to clash.

## The display (`GraphicsSettingsGroup`, `EftDisplaySettings`, `GameSettingsGroup`)

- **VSync** (`VSync`, off in the install): frames wait for the screen. (The game also holds the frame rate to a limit of its own, 120 in the install; not built.)
- **The field of view** (`FieldOfView`, on the game's Game tab): whole vertical degrees, 50 to 75, 50 unless set (74 in the install). With the sights up it is 15 less (`weapon-handling.md`).
- **How the window is shown** (`FullScreenMode`, kept as a figure): 0 the screen taken over, 1 a borderless window over all of it, 2 a window. The install has 1.
- **The resolution** (1920 by 1080 in the install).

The game puts a changed setting on when its screen is saved. The app puts each on as it is chosen. `PROVISIONAL`: a field of view changed in play eases to its new figure as a change of aim does; how the game's camera gets there was not read.

## Features: what is switched

The game has no such switches: these are the app's own, each by the user's word, and each kept in `local\options.json` beside the rest (`"features"`).

- **`CUSTOM FIRST PERSON HANDS`** (the user, 2026-10-13, night: "In the options for mechanics add an option for 'Custom First Person Hands' which can be enabled or disabled to not change with body-changing outfits. The fall back should always be the defualt tarkov outfit"). On, an outfit that has arms of its own for first person shows them there (`outfits.md`, "The arms for first person"). **Off, the hands do not change with what is worn**: in first person they are Tarkov's default arms whatever outfit is on, the Telnik's bare hands included; the outfit's body from outside, its shadow and its legs under the eyes are as before. It is on unless switched (`"custom_first_person_hands"` in the file; `--custom-hands on|off` on the command line has the last word, and is how a run nobody is at the keys of is told). Off as a run begins, no outfit's arms are read or fitted at all.

- **`SMOOTH FIRST PERSON ANIMATIONS`** (the user, 2026-10-14: "Add another switchable mechanic where we can either add interpolation across all first-person animations or disable it"). On, every animation of what is in hand (any weapon, empty hands) is drawn on curves through its clips' samples, where the engine joins them by straight lines; off, every one is drawn as the engine has it, the look all over a weapon included (`firing.md`, "How the look all over is drawn"). The drawing alone. On unless switched (`"smooth_first_person_animations"`; `--smooth-animations on|off`). On, each pose of what is in hand is worked out twice a step of the simulation, the engine's for the simulation to go by and the smooth one to draw (some 0.01 ms a frame, `firing.md`); off, once.

- **`ICON GENERATION`** (the user, 2026-10-14, evening: "Then, add icon generation to the switchable mechanics tab"). On, the inventory's pictures are drawn from each item's own model as the game draws them, once, and kept (`icons.md`); off, none is drawn or read, and each box has the plain shape of its sort, as before there were pictures. On unless switched (`"icon_generation"`; `--icons on|off`).

**Each switch is "restart required"** (the user, 2026-10-14, evening: "for better optimization, remember to add 'restart required' for certain settings to take effect to avoid for example the double animation rendering from smooth animations"). A run holds the switches it began with to its end (`Features`, a resource set once); a switch thrown on the tab is kept in the file for the next run and changes nothing now. So nothing is made, read or taken away in play because of one: the outfits' arms are read or not as the run begins, the icon studio is made or not, and the smooth pose is worked out or not from the first frame. Each switch's row says `RESTART REQUIRED`, faint; thrown away from what the run holds it is bright, with what holds until then ("on until then").

Everything else on that part of the tab is still the list, with a mark where a switch will be.

## Built (2026-10-12)

- **`trp-sim/src/bindings.rs`** (no engine types; a key is the game engine's own number for it): the bindings (`Controls`, `KeyGroup`, `AxisGroup`, `Source`), the manager's combinations (`Preset`: key states, the order, taking and giving way, the four ways of pressing with the double click's wait and the stuck release, the axes with their shared keys), choosing a key (`Listening`) and the rules for the same keys on two bindings (`Controls::key_chosen`, `press_chosen`, `axis_chosen`). Nine tests: a press and a release; the vault and the jump on one key; a held binding that does not hold; the longer combination having the key (Alt with T, Alt with a lean key, the sprint's key never given up); the crouch key with the wheel; the double click's wait; the axes and the slow lean taking the walk's keys; a key chosen; the same keys on two bindings.
- **`trp-sim` takes the commands**: `Input::sprint_toggle`, `sprint_end`, `lean_toggle`, `lean_end`, `step_toggle`, `step_end`, `breath_end`, `restore_pose`, beside the held keys it took before (which the recorder's scripts and the replay still use: the regression is unchanged, below). `Hands::set_field_of_view`. Since 2026-10-12, night: `Input::walk_toggle`, `walk_end`, `slow_lean`; `Movement::drop_backpack`; `Firearm::check_fire_mode`, `examine`, `force_auto_fire_mode`, `drop_backpack`; `bindings::install_controls`, which reads the install's bindings for the app and for `bin\byhand` alike.
- **`trp-data`**: `Install::key_bindings` keeps both variants and a variant's wheel; `axis_bindings`, `control_setting`, `display_settings`. Since 2026-10-12, night: `engine_axes` (the build's input settings, read by the engine's fixed layout for an axis), `axis_sensitivity`, `control_flag`.
- **The app**:
  - `keys.rs`: the install's bindings run through the manager every frame (`hear`); **every key the character takes is a binding** (it had been the inventory's keys and the fold only; the rest were written into `controller.rs`). The top of the display reads the keys off the bindings.
  - `options_view.rs`: the tab. Controls, **laid out as the game's own tab is** (2026-10-12, night; the user: "all binds are in a big scrollable box in the center, and 4 sliders on the bottom"): every binding the game shows in one box that scrolls, four columns with the game's shares of the width, the game's words for each binding and for each way of being pressed; under it the mouse's panel with the four bars and the two switches, by the game's words and within the game's limits; a click to choose keys, the question of how a binding is pressed (its four ways laid over the second key's cell), and a button that puts back the install's keys and mouse. Graphics: the four settings, and a button for what the install's own game has. Features: the list. **At the far right of the tab's own row, `QUIT`**, which ends the run (the user, 2026-10-12; `EXIT` at first, renamed by the user's word on 2026-10-13). **The list as the user had it changed on 2026-10-13:** the rows take two shades by turns (the second a little lighter), and a little room (8 pixels) is left before each binding that begins a group, over the game's own order, which is kept: the axes' and what follows them down to the speed's two; the weapon's handling from `WeaponMounting`; the weapons taken in hand from `PrimaryWeaponFirst`; what is worn and switched from `ToggleGoggles`; the other actions from `Nidnod`; the quick-use keys from `Slot4`; the screen's own from `MakeScreenshot`. The grouping is the app's: the game's list has none. (For an hour that day the list was also narrower, four fifths of the page and rows of 24 pixels, on a reading of "a little smaller"; the user had it put back as it was: the page's whole width and rows of 26.)
  - `cursor.rs` (the app's own, 2026-10-12, night): the pointer is hidden and held in the window's middle while the character plays, shown and kept inside the window while a screen is open, free with the free camera or with another window in front, and taken again when the window comes back to the front. On Windows the window is kept from going to look for a menu at a key pressed with Alt (`--alt-menu` leaves the system's way in place, to look at): the one command the system sends for that is answered and nothing else is. **The night's first cut also answered the letter the system makes of such a key, and with that no key pressed with Alt held reached the character at all** (the user, 2026-10-13: "I can't move when holding alt in the app now"): the engine's own account of a key going down waits for that letter. Put right the same day.
  - `hud.rs` (2026-10-13, by the user's word): at the top left, in the inventory screen's colours, **a row of the debug figures** (the movement state, the speeds, the stance, the lean, the weapon, the animator of the thing in hand), and under it with a small gap **the guide to the keys**: every binding the character acts on, in sections, read off the bindings as they stand, each binding that holds marked as held or as one press on and one off, a double click as `x2`. `F1` hides and shows the guide; it stands back while the inventory screen is open, and names the free camera's keys while that flies. They take the place of the lines of text that ran across the top.
  - What is changed is written to `local\options.json` and read at the start by a run someone is at the keys of. A run nobody is at the keys of neither reads nor writes it, and keeps its window.

**What this changed for someone at the keys**, all of it the install's own bindings:

- the sprint and the aim are one press on, one press off (they were held);
- the reload comes when `R` is let go, 0.3 s after it went down (it came as the key went down);
- the look at the magazine is `T` with `Left Alt` (it was `T` alone, which is the tactical device's);
- the stance is `C` with the wheel (it was `Left Ctrl` with the wheel), and `C` crouches as it comes up;
- the fire mode and the pistol's key are 0.3 s late, each having a double click on its key;
- D with A walks right, W with S forward; with `Left Alt` held, D and A do not walk: they lean slowly (since 2026-10-12, night; before, they did nothing here).

Each can be changed back on the tab (the sprint and the aim to `Continuous`; a double click moved off a key ends its wait).

**What the night of 2026-10-12 added for someone at the keys:**

- `Left Alt` with `D` or `A` leans slowly, and the lean stays where it is left;
- `Caps Lock` puts the speed to its least and back;
- `Z` twice drops the backpack; `L` looks the weapon over; `Left Alt` with `B` looks at its selector; `B` twice puts it on automatic;
- the mouse turns the view a quarter faster than it did (0.125 degrees a count for 0.1);
- the wheel turned two notches in one frame does nothing.

**Not built:** the bindings for what the controller does not have, each of which waits for a system and not for a key (the head's free look, `Nidnod` with the mouse's `LookX` and `LookY`, is built since 2026-10-13: `free-look.md`): blind fire's two; the left shoulder (`LeftStance`); mounting and the bipod; everything of a magazine as a thing of its own (`QuickReloadWeapon`, `NextMagazine`, `PreviousMagazine`, `UnloadMagazine`) and of the chambered round (`ChamberUnload`); optics (`ChangeAimScope`, `ChangeAimScopeMagnification`, the zoom's two, the zeroing's two, and the bar for the zoom's sensitivity, which is kept and does nothing); the knife, its fast strike and the grenade (weapons not told to be built); the pistol's fast draw (`QuickSecondaryWeapon`); goggles, the head's and the weapon's lights; the voice and the phrases' wheel (the tab's second part in the game); the watch; the screenshot, the console and the screen's own toggles. Also: the function keys' and Enter's own combinations, and Escape anywhere but on the inventory screen (`inventory.md`, "Escape"); the bindings the game makes out of others; a figure typed into a bar's box (the bars are slid); the frame-rate limit; a display chosen among several; the game's own art for any of it.

## Compared with what

- **The recorded keys, put through the manager as written here** (2026-10-12, night; `bin\byhand`, `keys_heard`): a recording made by hand has every key going down and coming up (recorder 0.6.1 on), and the manager is run over them with the install's bindings and each frame's own length. A binding whose doing shows in the recording on the frame it is done is set beside the frame the model gives it on:
  - **`R` tapped** (`local\traces\byhand4`, made for it by the user): down on frame 48, up on frame 55, and the reload's operation and its animator's first state on frame 85, **0.312 s after the key went down**. The model gives `ReloadWeapon` on frame 85. The wait is the game's: `QuickReloadWeapon` is the same key's double click, and the plain press waits out its 0.3 s and one frame more for the key to be seen up.
  - **the inventory's key** in four recordings of `local\traces\byhand3`: the screen opened or shut on the frame the model gives `Inventory`, all four times.
  - **Five of five on the game's own frame.** The rest of what the seventeen recordings' keys give (the trigger, the interact key, the quick-use keys' two each, the weapon slots') is listed for reading; the wheel is not on record.
- **A tapped `R` in the app** (a capture at 165 frames a second, `--tap 200:r:7`): the reload's first state is drawn 52 frames after the key, 0.315 s.
- **The regression** over the sixteen kept recordings is unchanged by the night's work (`after37`: fifteen reports byte for byte `after35`'s, the sixteenth differing in two lines that say how long reading the clips took); before that `after36` was byte for byte `after35`: the simulation's new inputs are not used by the replay.
- **Tests**: the nine above; the commands in the simulation against the install (`commands_do_what_the_held_keys_do`: a sprint pressed on and pressed off, and ended by a release; a lean pressed on and off); the options file written and read back over the install's.
- **Run, by scripted captures** (`--tap` presses keys through the bindings in a capture that holds none of its own):
  - the tab opened by a click, its three parts, the list scrolled;
  - a key chosen (`Prone` on `R`: both it and the reload keep the key, pressed differently), and one that clashes (`WeaponMounting` on `Left Alt`, pressed as the breath is: the four ways are offered);
  - VSync off, the field of view at 60 and a window of 1366 by 768, each put on as chosen;
  - `W` held and `Left Shift` tapped: sprinting 23 frames on, still sprinting with the key up, walking again after the second tap;
  - the right button tapped: the sights still up 170 frames after it was let go;
  - `R` tapped: the reload's first state 53 drawn frames after the key went down (0.32 s);
  - `T` with `Left Alt`: the look at the magazine; `C` tapped: crouched; `Q` held: leaning.
- **Run on 2026-10-12, night, by scripted captures:** the tab in its new layout, its list scrolled to the end; the sensitivity's bar slid (0.700 to 2.800, drawn where it stands); the inverted Y axis switched; `EXIT` clicked (the run ends at once, with nothing written); `Left Alt` with `D` (the lean at 4 degrees and staying with the keys let go); `Caps Lock` in a walk (the speed at none, 0.86 m/s); `Z` twice with a pack on (the gesture from three frames after the second press comes up, for 1.2 s, the pack on the ground and its slot empty); `L`, `Left Alt` with `B`, `B` twice (the animator's `LOOK`, `FIREMODE CHECK` and `FIREMODE` within two frames of the key).
- **The pointer and the keys under Alt, measured on a real window** (`local\window_keys_test.ps1`: the app run as someone at the keys runs it, in a window with a title bar, `W` sent sixty times with Alt held as a keyboard sends it, the system asked for every showing and hiding of the pointer, and the app's own log read for where the feet went): **with the window's own answer to Alt left in place (`--alt-menu`), the system showed the pointer sixty times and hid it sixty times**, once a key; **with the app's answer, never; and in both the character walked 4.7 m on those keys.** At rest the pointer is hidden and held to one pixel in both. A borderless window has no menu to look for and does not flicker either way. (The first cut of 2026-10-12 was measured for the pointer alone, and so its fault, the keys not arriving, went unseen: the captures press keys inside the app, past the window.)
- **Not run:** the screen taken over and the borderless window (a capture would have changed the user's screen); the two buttons that put back the install's; the pointer taken again after another window was in front; a bar slid by hand; anything by hand.
- **The third switch and "restart required", 2026-10-14, evening** (scripted captures, `local\shots\s49`): the part with three switches `ON`, each row saying `RESTART REQUIRED`; a click on the third (`--drag 40:242:40:242` after the part is open), and it says `OFF`, its note bright and "on until then", and the run's pictures stay as they were. `--icons off` starts with every box its sort's plain shape and a line saying so; `--custom-hands off` starts with a line for each outfit whose arms are not read. The file's test writes and reads all three. **Not run:** by hand, nor a run started after a switch thrown on the tab (a capture neither reads nor writes `local\options.json`).
- **The second switch, 2026-10-14**: the part with both switches `ON`; a click on the second (`--drag 40:207:40:207` after the part is open), and it says `OFF`; a reload after the screen is shut is drawn on the engine's lines (`firing.md` has the figures). The file's test writes and reads both.
- **The features' switch, 2026-10-13, night** (`options_view::Features`, `CustomHandsSwitch`; `body_view::show_outfit`): by a scripted capture, the `FEATURES` part with the switch `ON`, a click on it (`--inventory --drag 1490:78:1490:78 --drag 400:125:400:125 --drag 40:172:40:172`), `OFF`, and with the screen shut the Sunrise suit's wearer has Tarkov's default gloves; `--custom-hands off` starts so. The file's test writes the switch off and reads it back, and reads a file that has none of it as on. **Not run:** by hand; the user's own `local\options.json` was not written by any of this (a capture neither reads nor writes it).
