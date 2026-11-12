# Changelog

One number covers the whole of TRP. It is raised with every change. Versions before 0.251.0 are not listed here.

## 0.262.2

- Body armour and an armoured rig can no longer be worn together, as in Tarkov. With an armoured rig on, the BODY ARMOR slot takes nothing, and with body armour on, an armoured rig is refused. A rig with no armour of its own still goes on over body armour.

## 0.262.0

- The SPT folder is now named by the `SPT_DIR` environment variable, or kept in `TRP/local/spt_dir.txt`. If you set TRP up with an earlier release, run `Install.bat` once more: it asks for your folders again.
- This repository now holds only what building, running and changing TRP needs. The probe is its one command-line tool.
- Every section of the options menu is open when TRP starts. A click on a section's name still folds it.
- HUD's NOTIFICATIONS no longer has an INCOMPATIBLE AMMO box: that warning is always shown, as Tarkov shows it.

## 0.261.0

- The options menu's SKILLS page shows each skill's name and bar alone: what a skill does is on the SKILLS tab.
- The bar across the bottom of the options menu is gone. APPLY stays at the lower right, with CLEAR KEY beside it on CONTROLS, and the (i) beside it is golden while a change waits for a restart and grey otherwise.

## 0.260.0

- The options menu says less: the grey line beside each page's title and each section's name is gone, and so are the line under each setting and the footer's text for the setting under the mouse. Buttons that do something (QUIT, RESET TO DEFAULT, APPLY, DEFAULT) still say what they do there.
- FEATURES keeps the line beside each switch that says what it does. Its sections are now EXPERIMENTAL, CHARACTER and THE RANGE, and CHARACTER NAME is inside CHARACTER.
- HUD's sections are now IN-RAID INTERFACE, SCREENS and NOTIFICATIONS.
- On the SKILLS page each skill shows Tarkov's own description of it, read from your install (the words the wiki gives), in place of TRP's own wording. The footer no longer says that nothing is earned there: what you earn in play shows on the page and is kept.

## 0.259.1

- Turning the view costs less: the drawn ribcage is worked out from its six bones and no longer by posing the whole body twice a frame.
- Walking up to things hitches less: a picture brought up to a finer level is handed to the graphics card without being copied first.
- Sprinting and aiming no longer lay the whole interface out on every frame: a stamina bar's width is written only when its edge moves a pixel.
- A scope's picture is whole the first time a scope is raised in a run (the range's distance signs were missing from it for a moment), and the long frame just after the start is gone. The start takes about 0.1 s longer.
- A scope's camera leaves out the body pieces and clothing it cannot see and keeps its lights in one cluster; its picture is the same. It follows the MSAA option when that is changed.
- The OVERALL tab's figure rests while its tab is shut, and the quick bar and the inventory's health panel are worked out only when something they show has changed.

## 0.259.0

- LEDGE CLIMBING is taken out until its move reads right: the character no longer jumps up to, hangs from or climbs a high ledge. Its switch stays on the FEATURES page, dim and marked NOT BUILT, and cannot be turned on; an options file that had it on is read as off.
- The range's three climbing blocks and their starts (`--start climb-low`, `climb`, `climb-high`) are gone with it.

## 0.258.0

- LEDGE CLIMBING, a new switch on the FEATURES page (off as the app comes, and not Tarkov's): hold forward and the jump key before a ledge too high for the game's climb, up to 2.8 m, and the character puts its weapon away, gets up onto it and takes the weapon in hand again.
- With four clips of your own in `local/animations/ledge` (a jump to a hang, a hang, a climb with the feet on the wall and one from a free edge) the move is played from them: the character jumps, catches the edge and hangs; forward climbs and back lets go, and a ledge too low to hang from is climbed straight from the ground. Without the clips the move is made from the game's own climb with the arms reaching for the edge.
- The range has three blocks to try it on, 1.6, 2.2 and 2.6 m high (`--start climb-low`, `climb`, `climb-high`).

## 0.257.0

- A weapon put down from the inventory is seen in the air from the moment the game would show it, and heard as it lands. It used to appear only once it lay on the floor.
- A dummy's cry of pain or death stays where the dummy is as you turn or walk away, instead of following you.
- A character's ears no longer poke through a balaclava or any other face cover: the head is drawn in its under-cloth shape, as the game does.
- The MASTERING search field and the tag window's field edit as the game's do: click or use the arrows, Home and End to put the caret anywhere, hold Shift to select, Ctrl+A selects all, Delete works, a double click selects a word and a drag selects. The tag field's caret blinks and its first click selects all, and the search field takes 20 letters at most.
- A stimulant's tooltip updates its countdown in place instead of being rebuilt every second, so its frame no longer blinks as the seconds move.
- Two small costs are no longer paid every frame: the model keys of the things on the ground, and the search for each glowing weapon's item.

## 0.256.2

- With TARKOV UI on, the plain interface's own figure of the character no longer takes part in every frame. It was built for the plain gear page and kept live though TARKOV UI never draws it.

## 0.256.1

- Weapons worn but not in hand cost less each frame: their copies on the inventory's two figures rest until a picture of the figure is drawn. With a second rifle and a pistol worn the frame is about 5% shorter.

## 0.256.0

- The MASTERING search field shows a blinking caret after what is typed, and selects all it holds when it is clicked, as the game's does.
- A text caret is never thinner than a pixel. At some window sizes it fell between two pixels and was not drawn.
- The first start writes the games' folders into the same `local` folder the options and the logs use, wherever the app is started from.
- Weapons hung on the body cost a little less each frame.

## 0.255.0

- The dead blow hammer, the hand scythe and the Chained Labrys hit with their own sound, one on a body and another on anything else, in place of the struck surface's.
- Seen from outside, the body's reach to draw a weapon or put it away plays faster with the weapon's skill, as the hands' already did.

## 0.254.0

- Rounds held over a magazine that will not take them say why at the pointer: "Magazine is full" or "Wrong ammo type".
- A grid whose filter refuses a thing says "Item is incompatible", as a slot already did.
- Ctrl+click with nowhere to put the thing raises the game's warning notice, "No space for this item".

## 0.253.0

- Each weapon keeps its own fire mode, chosen sight, sight mode, zeroing and zoom. Two weapons of the same build shared them, and they were lost 30 seconds after a weapon was put down.

## 0.252.1

- A knife stays in the hand while sprinting. It vanished a quarter of a second into every sprint.

## 0.252.0

- A knife stroke is refused while sprinting or crawling, as in the game.
- A knife hit plays the struck surface's particles, light and sound.
- A round loaded into the chamber by hand is drawn from the moment the hand shows it.
- The quick bar and the slot views keep a weapon's picture until its reload is over.
- The encumbered and over-encumbered notices show once the weight passes its limits.

## 0.251.0

- The source is reorganised. Every crate's modules sit in feature folders, and long files are split into folders of small ones.
- The command-line tools moved to their own crate, `trp-tools`.
- The FEATURES page shows only SIMPLE XP GAIN and the experimental switches, and HUD shows only COMPATIBLE ITEMS HIGHLIGHT. Every other switch still works from the options file and from `--feature`.
- The documentation is rewritten: a short README and AGENTS.md, a feature list, and a generated index of every source file.
- Memory no longer grows each time a weapon is put on or rebuilt.
- Weapons that are worn but not in hand cost a little less each frame.
- "Tunnel vision", "Stunned" and "Berserk" show their proper names.
- The X-17 and the AK-50 have a picture on the MASTERING page.
- A tooltip whose words change is made before the frame is drawn, which should end its blink.
- The run's log counts entities, meshes, materials and pictures every 30 seconds.
