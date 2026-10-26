# The inspect window: its figures and its model

The window that shows a thing (`InfoWindow`, `ItemSpecificationPanel`, `ItemInfoWindowLabels`;
its frame, caption, kind, weight and description in `inventory.md`, "The window that shows a
thing") lists the thing's figures under its preview and draws its model there, turned by a drag
and brought nearer or farther by the wheel. Read from the decompiled code and the install; the
user, 2026-10-21: "the inspect menu should display more information about the weapon/armor/ammo
inspected. Make it have the same 3d model as the workbench."

## Where the figures go

The window's description panel (`Inner/Contents/DescriptionPanel`, a column) holds, in order: the
**column of bars** (`CharacteristicsPanel`, a column 10 in from its top, its rows 4 apart; first the
window's own durability row for a thing that has durability, then a bar row for each attribute shown
as a bar), the **compact grid** (`CompactPanel`, a `FlexibleGridLayoutGroup`: cells 23 high, 4 apart
down and 12.7 across, filled down each column before the next), the parts' panel and the
description.

**What is listed** (`RecreateAttributeBars`): the thing's attributes in their order, each where its
kind of display puts it: a bar (`FullBar`) in the column, a compact row (`Compact`) in the grid, a
row with a dropdown (`CompactWithTooltip`) in the grid after all the compact rows; those of the
`Special` kind (durability and its greatest) nowhere. Attributes a thing is made with "safely" are
left out where their base is nought as it is made. Before the compact rows comes **ARMOR POINTS**
wherever the thing or anything in it has armour of a class above nought: the sum of those armours'
present durability, to one place. A part is given a dropdown of the weapons it fits and a plate one
of the armour that takes it (`CompatibleWith`, `CompatibleWithArmor`: not built).

**How many columns** (`ProcessCompactCells`): two, each the grid's width less the gap between, by
half; one where any compact row would not fit its cell: 30 units, its name's width and, where it
shows a value, the value's width and 5 more, above the cell's width. The compact rows are made
first and the dropdown rows after them (`RecreateAttributeBars`, two lists of views on the same
grid), so the dropdown rows follow every compact row in the grid. Each cell is as wide as its
columns share the grid (`FlexibleGridLayoutGroup`, the grid's `FixedColumnCount` the columns: 2 in
the window), 23 high, filled down each column first (the grid's vertical axis) from the top left;
the grid as tall as its rows (`GridLayoutGroup`'s preferred height: its rows, 4 between).

**How tall the window is**: its own `LayoutElement` asks 670 by 500 and its `ContentSizeFitter`
fits it to what it asks, which is at least what all it holds needs (Unity takes the greater of a
thing's least and its preferred size): the preview gives up its room down to its least (300)
first, then the window grows. A weapon's window is about 685 high.

**A row** (the window's templates: a bar row `CharacteristicPanel`, 23 high; a compact row
`CompactCharacteristicPanel` and a dropdown row `CompactCharacteristicDropdownPanel`, each a row of
the grid's cell; each a `HorizontalLayoutGroup` that sizes what it holds): the attribute's icon in
its place in the row (`StaticIcons.ItemAttributeSprites`, by the attribute's name; the row closes up
where it has none), its name (its key, `ItemAttributeNames.GetName`, in the install's words, upper
case) and its value where it has one to show (`IsTextValueDisplayable`), each as wide and as high as
its font draws it and the rest of the row shared between the two (each may grow alike): a bar row's
value left in its half, a compact row's at the right; a dropdown row has its arrow after its value,
and says its lines when the mouse rests on it (not built). **Coloured values** (`Colored`): a positive value has a "+" before it and the row's
increasing colour, a negative one the decreasing colour (the two swapped where less is better). **A
bar** (`CharacteristicPanel.SetValues`): with t the place of a figure in the attribute's range (which
may run downwards), the bar's fill (`Full`) reaches the lesser of t(base) and t(base and its
change), its change (`Delta`) the rest up to the greater; the change is drawn with the row's
subtracting picture where it lowers the figure (or raises it on a range that runs downwards) and in
the adding or the subtracting colour as it makes the thing better or worse (less being better where
the attribute says so).

## A weapon (`Weapon`'s attributes)

The durability row (`DurabilityPanel`): "present/greatest (the template's)", the greatest in red
under 10 (a colour tag), its fill and its border in the row's low colour at 15% of the template's
durability or less, else its own; the fill to the present share of the template's durability and
the change to the greatest's. Then the bars, each over every part fitted at any depth:

- **ERGONOMICS** (0 to 100): base the template's ergonomics, rounded; its change the parts'
  ergonomics summed over the base (at least 1); written as the total held within 0 and 100.
- **ACCURACY** (`CenterOfImpact`; its range from the greatest centre of impact of any barrel and a
  tenth, down to a thousandth; less is better): base the barrel's centre of impact (the weapon's own
  without one); its change minus the parts' accuracy summed, in hundredths, and the barrel's
  deviation at its durability against its deviation at the template's, over the greatest deviation
  less the latter. Written in minutes of angle: the base times its parts' change, times the round's
  factor (100 over 100 and its accuracy where that is positive, else 100 and its accuracy's size
  over 100), times the barrel's deviation at its durability, times 100 over 2.9089, to two places
  at most, and the install's word for MOA. The deviation (`GetBarrelDeviation`, a quadratic curve
  from m down to 1): with c the deviation's curve and m its greatest (100 where none, both the
  barrel's or else the weapon's), the curve's parameter s for a durability d is
  (√((100 − 2c) d + c²) − c) / (100 − 2c) (d / 2c where 100 = 2c), and the deviation
  (1 − s)² m + 2 s (1 − s) c + s². A weapon as made, at its template's durability, has no change of
  its own from it.
- **SIGHTING RANGE** (0 to 5000): the greatest of the weapon's and its parts'; "N/A" where none.
- **Recoil up** and **Recoil back** (0 to 1000; less is better): the template's forces, their change
  the parts' recoil summed in hundredths (a folded stock's left out), written rounded.
- **Velocity** (0 to 1500): base the speed of the round it would fire (the chamber's, else the
  magazine's first, else the template's own), its change the parts' and the weapon's own velocity
  summed in hundredths; written whole with the install's word for m/s.

Then the compact rows: **TYPES OF FIRE** (its modes in the install's words, ", " between),
**CALIBER** (the calibre's key in the install's words), **FIRE RATE** (with "rpm" in its words),
**EFFECTIVE DISTANCE** (with "meters" in its words); a weapon's size change, a repair buff and the
discard limit not built.

## A container (`CompoundItem`)

Before anything else of a thing with grids (its base class makes it first): **CONTAINER SIZE**, its
grids' cells summed, where there are any. An armoured rig thus lists ARMOR POINTS, CONTAINER SIZE,
then its penalties (the user's capture of the Bagariy: 232, 25, -9 %, -3 %, -4 %).

## Armour (body armour, armoured rigs, helmets, plates)

In the order they are made: from the thing's own armour where it has a durability and a class
(`ArmorComponent`): **MATERIAL** ("Mat" and the material's name, in the install's words) and, where it
is light or heavy, **ARMOR TYPE**; from its penalties (`EquipmentPenaltyComponent`, where it has the
penalty or any plate's slot), each its own and its plates' summed, "%" after, coloured: **CHANGE
MOVEMENT SPEED**, **CHANGE TURNING SPEED**, **ERGONOMICS**; **EYES PROTECTION** where it has any
(`BlindnessProtection`, a whole percentage). Then, for what holds armour (`ArmorHolderComponent`),
moved to the end: **ARMOR ZONE(S)** (its zones' names in the install's words, no value shown; a
dropdown with more than one), **RICOCHET CHANCE** for headwear and face covers (by the head armour's
ricochet's top: high above 0.3, medium above 0.15, else low, in the install's words), **ARMOR CLASS**
(the first armour's class; a dropdown with more than one, listing each plate). ARMOR POINTS before
them all.

## What a dropdown row says

Its arrow (`_dropdownImage`) with the mouse on it shows a tooltip (`CompactCharacteristicDropdownPanel.
ShowTooltip`): the attribute's lines where it has them (`MultiLineInfo`, in the `MultiLineTooltip`),
else its words (`StringValue`, in the simple tooltip). **A round's penetration**
(`MultiLineInfo(penetrationPower)`): for classes 1 to 6 the class's mark and the install's words for
the chance through new armour of the class (`RealResistance` at 100 of 100,
`GetPenetrationChance`: under 20 very low, under 40 low, under 60 medium, under 80 high, else very
high; each word with its colour in the install's locale). **Armour's zones**: its zones' names, a
line each. **Armour's class** (`MultiLineInfo(item, false)`): ARMOR POINTS, a break, then each plate
in its own armour slots, the locked first, each group by class downwards, a break between them; each
its class's mark where the class changes (else as much room), its slot's name, " - ", its
durability and its greatest to a place. The marks are a TextMeshPro sprite asset's
(`Sprite Assets/armor_classes`: its character table names `armor_classes_1` to `_6`, its glyph table
points each to one of the engine's sprites, 20 by 19).

## The parts' panel

On for a thing with slots or a weapon with chambers (not a rocket launcher): a slot view
(`ModSlotView`, 64 square, a prefab of `resources.assets`) for each of `AllSlots`: the thing's own
slots in order, then for each of them its part's slots, parts of parts in turn after their parent's
(`Flatten`), a weapon's chambers last. An empty slot shows its empty border, its ground, its picture
(`Resources/Slots/<the slot's name>`, at its own size in the middle; a launcher's are all
`grenade_patron_in_weapon`, an armour slot with none `armor_plate`), its name (`slot.Name`, in the
install's words, upper case; `Slot.Name` is the first kind of weapon part, `EWeaponModType` in its
order, that the slot's own name holds, else that name: `mod_mount_004` is `mod_mount`, "MOUNT",
and its picture `Slots/mod_mount`; centred, an ellipsis where it is wider than the slot,
`m_overflowMode` 1) and its filters' marks (`SetFilters`: the kinds of part it takes, a row
from the lower left, 2 apart); a full one its full border and its part's view, which is captioned
with the slot's name where it is an armour's plate or insert in an armour's slot
(`GridItemView.ItemShortName`). **The part's view is the grid's** (`ModSlotView.
CreateItemViewKernel`, `ItemUiContext.CreateItemView`: a `GridItemView` in the inventory), not the
equipment slot's: its count written small (`<color>count</color>/most`), no mastering, no
durability bar; its caption right-aligned and truncated at its end where it does not fit
(`m_overflowMode` 3: "FRONT PL", "GROIN (RE"), its picture at its own size, shrunk to fit the slot
(`UpdateScale`); its marks at the lower left: a plate that may be taken out (not in a slot, or of a
class in a slot not locked, `ItemViewStats.IsMoveAblePlate`) its armour class's
(`Mod Types/icon_type_mod_armor_plate_<class>`), anything else the part's class's ground and kind's
mark, an insert's `armor_plate` on `armor_plate` (`GetModClass`: functional, gear, master,
auxiliary as gear, armour plate). An armour's built-in insert has no model to draw; its picture is
its prefab's own (`PreviewPivot.Icon.overrideIcon`, `icons.md`). **Locked in a raid** (`GetModLockedState`, `ModLock`: the slot at
half its alpha, out of the mouse's reach): a slot that is locked (`locked` in its filter: built-in
inserts), a plate in armour worn, a part not to be changed in a raid, a vital part's slot; an
unchecked chamber (shown with its own picture, its part hidden) is not built. The grid
(`GridLayoutGroup`): 64 square, 3 apart, as many to a row as fit, the block in the middle of the top;
the window at least as wide as six slots or a third of them ask with the paddings. Its parts'
pictures are asked for as the window opens, as every view's is.

**Fixed 2026-10-24** (0.113.0, against the user's captures of the SPEAR and the AS VAL): an empty slot's
picture is taken by its `Name` as above (it was taken by the slot's own id, so `mod_mount_000` and the
like had none); a slot's words are drawn in their own font (the slot's template is `resources.assets`',
the window `sharedassets49`'s: the grafted texts' font, material and pictures now count in the window's
table of files, where they had fallen to the engine's own monospaced font); a part's caption is cut at
its visible letters only (TextMeshPro tests a line's room at its visible characters, so the padded
name's trailing space no longer cuts "SPEAR 6.8" to "SPEAR 6.").

## The buttons

Its row of buttons (`InitInteractionButtonsPanel`): the thing's menu's lines but Inspect, as the
right-click menu has them in a raid, each a copy of the window's own template (Discard's own, the
wish list's own, the rest), in its grid (three columns of 175 by 18, 5 apart, filled row by row, the
block in the middle), dimmed to 0.3 where it cannot be done. A click does the line; the wish list's
opens its own lines (the app: in the menu, where the mouse is).

**Under the mouse** (read and built 2026-10-24, 0.115.0; `health_view::light_info_buttons`): each
template's Unity `Button` has no transition (`m_Transition` None), so nothing changes on a press; its
`ContextMenuButton` (`SimpleContextMenuButton`) sets its ground (`_backgroundImage`, the `Background`
plate) to its selected colour as the mouse comes on (`OnPointerEnter`) and back as it leaves, at once:
the plain template (0.169, 0.176, 0.180) to (0.204, 0, 0), the wish list's (0.067, 0.200, 0.259) to
(0.129, 0.310, 0.392), Discard's (0.282, 0.051, 0.051) to (0.204, 0, 0) (the right-click menu's own).
One that cannot be done still takes the mouse (`SetUnlockStatus` keeps its raycasts) and lights at its
0.3. No sound. Not built: a `Blocked` button (its lines open) kept lit; the reason's tooltip on one that
cannot be done.

## The description

(Read and built 2026-10-24, 0.115.0; `health_view::info_scroll`, `inventory_view::scroll_info_texts`.)
Its `Scroll Area` (`DescriptionPanel/DescriptionPanel`) is a `ScrollRectNoDrag` over the text: up and
down only, elastic with an elasticity of nought (back within its ends on the same frame: it stops at
them), no inertia, **10 units a notch** of the wheel, the area its own view, its bar **hidden while
everything shows** (`AutoHide`: the content no taller than the view by a hundredth; the view not
widened). The content (`Contents`, at its preferred height) is the text as its font lays it out at the
area's width less its margins, with its margins above and below. The wheel works over the area
alone, only while the bar shows (`ScrollRectNoDrag.OnScroll`); the text cannot be dragged (its drag
handlers are empty); the bar's handle (6 wide, its share of the bar the view's of the text) is dragged,
and the bar held off it steps a handle's length toward the press each frame (`Scrollbar.ClickRepeat`),
the handle tinted 1, 0.961 under the mouse, 0.784 held, over 0.1 s. A window opens at the top (a new
copy of its template) and keeps its place while it is open. `PROVISIONAL`: a text not in one of the
game's fonts is taken to fit. **The windows are above the quick-use bar to the mouse** as they are
drawn above it (they had ranked below it: anything of a window over the bar lost the mouse to it).

## A weapon's mastering

For a weapon, its mastering row (`MasteringCharacteristicPanel`): "WEAPON MASTERING (0N)" with N its
level and one, its points into the level over the points the level takes (its mastering group's
`Level2` for level 0, `Level3` after), the fill to their share; its badge under the preview with
"0N", and the preview's lower left corner (`_masteringBg`). The app keeps the level (`--mastering`),
not the points: they are written as 0 (`PROVISIONAL`).

## Stretching it

Its edges and corners (`StretchButtons`: 10 units outside its sides, 16 square at its corners) stretch
it (`StretchArea.OnDrag`): the mouse's travel along the sides it stretches, out for the right and the
bottom, added to its preferred size (its `LayoutElement`'s, 670 by 500 as made), held within nought
and its most (`StretchWindow._maxSize`, 1800 by 1024 in the install: the script keeps Odin's data
first, so it is found as the two figures before its pointer to the window's element); laid out
again, the window as large as that or as what it holds asks; its preferred size what it came to,
and the drag goes on from where the edge is. The window moves so that its other side stays, and
back onto the screen. Its grids, bars, preview and description follow its width.

## Where it opens

As a window of its kind (`inventory.md`), at its own size once laid out: alone, in the middle of the
screen; and moved back onto the screen (`CorrectPositionResolution`).

## A round (`AmmoTemplate.GetCachedReadonlyQualities`)

Each where its base is not nought: **MAX AMMO DAMAGE**; **AMMO PENETRATION POWER** (a dropdown: its
lines the chance by armour class, "What a dropdown row says"); **CALIBER** (always: the round's own `Caliber` in the
install's words, which has none for it, so the key itself is written, "Caliber762x39", as the
game's `Localized` does; a weapon's drops "Caliber" first); **AMMO PROJECTILE COUNT** where
more than one; **BULLET SPEED** (with m/s); **ACCURACY** (its accuracy to one place and "%",
coloured); **RECOIL** (coloured, less is better); the chances to cause heavy and light bleeding (whole
percentages).

## The model

The window's preview is the same `WeaponPreview` as the edit build screen's (taken from a pool of
eight), drawn into a picture the size of the preview's own (`CameraImage`, at twice its size) over
the panel's ground: the thing's model as its loot (with its parts), its `PreviewPivot` placing it,
**turned by the pose of its icon** (`PreviewPivot.Icon.rotation`; the edit build screen turns a
weapon (0, −90, 0) instead), its own lights off. **The camera is fitted** to the model's bounds
(`PlaceInClosestPosition`): from 3 m behind, stepped 0.15 m nearer at a time (at most 21 steps)
until the bounds are no longer wholly in sight, then backed off 0.01 m at a time until they are,
held between 2.7 and 0.45 m; that place is kept for the template for the rest of the run (a static
cache). **A drag** on the preview's panel (`ItemInfoWindowLabels.Rotate`: `Rotate(dx, dy, 0, 0)`,
the pointer's travel in the screen's pixels) turns it back by 0.3 of a degree a pixel across and
tilts it by 0.3 of a degree a pixel up, the tilt's sense as the model faces, the tilt not held;
**the wheel** over the panel (`Zoom`: a notch 0.12 m) zooms as the edit build screen's zoom
(`workbench.md`, "The preview"). The loader shows until the model is in (`SetupItemPreview`'s first
call; `Show` turns it off first).

**The scopes' glass** (read and built 2026-10-24, 0.114.0; `loose::left_out`): a sight's lens, its
collimator's dot, its glass and its back lens are drawn by the sights' own shaders, which in the preview
draw next to nothing. The window's `CameraImage` turns the global keyword `WeaponPreview` on for the
preview's camera alone (`SetupPreCull`, put back in `RestoreRenderSettings`), and only `CW FX/OpticSight`
reads it: its preview variant discards every pixel, so an optic's lens draws nothing. A collimator's dot is
added light from its mark sampled along the lens's line from the eye: from beside the weapon that falls
outside the mark (clamped, all but clear), so nothing (inferred, not drawn here). The glasses
(`Custom/OpticGlass`, `EFT/CollimatorGlass`) add a reflection and a glint at a tenth's strength: about
nothing (`ESTIMATE`: left out). The back lens is opaque black and stays. Drawn as a plain material each was
an opaque pane of its `_Color`: the Velociraptor's RMR's glass a white pane seen edge on (the "white
outline"), its dot a red disc, the Tango's glass a white streak along its eyepiece. The edit build screen
draws straight to the screen with the keyword off, so there an optic's lens is its normal dark picture
(`PROVISIONAL`: left out the same). On the ground the collimators and glass are left out, an optic's lens
kept. An icon's are drawn as they stand by the game too (`ShaderReplacer`: no `_Icon` lens shader, each an
opaque reflective one of its colour): icons are unchanged.

**The pool's own preview** (read 2026-10-24, 0.113.0; `probe inspectpreview`, `edit_build::InspectPreview`):
the pool is `UIContext/Previews` of the preloader's scene (`PreloaderUIScene`, `WeaponPreviewPool`, eight),
its template `WeaponPreviewTemplate`, a prefab of `sharedassets49.assets` on layer 19. **Its camera**
(`Weapon Camera`, 1 m behind the template's middle): 22 degrees up and down, planes 0.01 to 1000 m, cleared
to nought, HDR in half floats (`HdrModeSwitcher`), deferred. **Its lights** are the two its
`CameraLightSwitcher` puts on as it draws and off after (the prefab keeps them off): "Directional light",
white at 1.8, from above and a little away from the camera, and "Some light", (0.75, 0.90, 1.0) at 0.8,
from below; directional, no shadows, children of the template, not turned with the model (a third,
"Additional light", 0.7, is off and not listed). Its `_weaponPreviewLights` is empty, so
`DisplaySettingsChangedHandler` moves nothing. **The ambient is black** around its picture
(`CameraImage.ForceInterfaceRenderSettings`: flat, black, intensity 1). **Its tonemap** is Standard Assets'
`Tonemapping` of the type 3, photographic, exposure 2 (`1 − 2^(−2c)`, the published curve). Also on the
camera, not built: Cinematic Effects' ambient occlusion (intensity 2.01, radius 0.05 m) and `Undithering`.
The model's own material is the item's (`p0/Reflective/Bumped Specular SMap`, with `_SpecColor`,
`_Glossness`, `_Cube` and `_ReflectColor`), drawn here as the app draws every model (Bevy's surface;
the shading's part two is not built). **As built** (0.113.0): the template's two lights (intensity as the
edit build screen's are put) and near plane, the black ambient, the photographic curve on the picture in
the game's numbers (`preview_tonemap`, `PROVISIONAL`: the build's own pass not disassembled); the field of
view, place and fitting as before (the edit build screen's camera has the same 22 degrees). Before it the
edit build screen's eleven point lights within 0.35 m of the model lit it: hot spots by them, black
elsewhere (the user, 2026-10-24: "very shiny, or has no lighting at all"). Looked at beside the user's
captures (`local\shots\sheet_preview_s122.png`): even and matte, as the game's.

## Built

**0.99.0** (2026-10-21): the figures and the model, with `TARKOV UI` on.

- **The rows**: the three templates read from the window's own pointers (`trp_data::screen::
  info_rows`), their colours and the bar's pictures from their scripts (`row_look`), the
  attributes' icons from the static icons (`attribute_icons`); a copy of a template for each row
  put into the window and laid out by the game's layout, the grid by its own rule
  (`health_view::info_built`); the figures from the install (`trp_sim::item_attributes`).
- **The model**: each window's own camera on a layer of its own (four, as four windows of the kind
  may be open), its picture twice the room, the model put in as the edit build screen's is
  (`edit_build::put_model`, shared), fitted, turned and zoomed (`info_preview.rs`); the camera
  draws only for a few frames after what it sees changes; a model is let go of as its last window
  shuts.
- **Seen** at 1920 by 1080 (`local\shots\info_akm_s110.png`: the AKM's durability row, six bars and
  four compact rows, its model; `info_ps_s110.png`: a round's four rows, the penetration's
  dropdown last; `info_gzhel_s110.png`: the Gzhel-K's ARMOR POINTS, penalties in red, zones and
  class as dropdowns; `info_drag_s110.png`, `info_wheel_s110.png`: a drag and the wheel). Not set
  beside the game's. **Tested**: the figures (`trp-sim/tests/attributes.rs`), the rows' templates,
  colours and icons read (`trp-data/tests/install.rs`).

**0.100.0** (2026-10-21; the user: "all attachments/avaible slots are displayed when inspecting",
"You can resize the inspect window to certain amounts", "the little v arrows inside the inspect
display more information when hovered", "The spinner when double clicking to inspect does not
spin", with captures of the game's windows of an AKM, a round, an armoured rig and body armour;
the buttons and the mastering row asked and answered "Yes, as in a raid"): the parts' panel, the
buttons, the mastering row and badge, stretching, the dropdowns' tooltips, CONTAINER SIZE, the
window opened at its own size, its loader turning (`ESTIMATE`: 1.1 turns a second, as the edit build
screen's). **The rows' words aligned as the game's**: TextMeshPro takes a text's older alignment
(`m_textAlignment`) over its two newer fields where it is not marked converted; read so, a bar's
value lies at its right and the rows' words on their middle line, as the captures show.
**Compared with the user's captures**: the AKM's slots in the game's order (fifteen, the same); the
Bagariy's figures (ARMOR POINTS 232, CONTAINER SIZE 25, MOVEMENT SPEED -9 %, TURN SPEED -3 %,
ERGONOMICS -4 %, ARMOR CLASS 5); a penetration of 35's lines (very high through I to III, low
through IV, very low through V and VI: `a_round_s_penetration_lines`). **Seen** at 1920 by 1080
(`local\shots\info_akm_s111.png`, `info_ps_tip_s111.png`, `info_gzhel_s111.png`,
`info_bag_s111.png`, `info_stretch_s111.png`).


## The discard limit (`Item`'s own)

Before any other row of any thing: **CANNOT BE DISCARDED IN RAID** (`EItemAttributeId/LimitedDiscard` in the
install's words), compact, where the template's `DiscardLimit` is 0 or more and the globals have the limits on
(`DiscardLimitsEnabled`): "> " and the limit, its value empty at 0, never coloured.

## Medicine, food and drink (`Meds`, `FoodDrink`; read and built 2026-10-26, 0.139.0)

The user, 2026-10-26: "Medical items have no inspect details. ... Read what the game's inspect window lists for
medicine, food and drink: the resource, the use time, what it cures, and a stimulant's buffs and side effects with
their times." Every row is compact, in this order (the components' order as the item is made):

- **HP RESOURCE** (`MedKitComponent`, only where the template's `MaxHpResource` is above 0): the present resource
  cut to a whole, "/", the greatest, a space and the install's word for HP ("1080/1800 HP"). Food and drink have no
  resource row (their resource is on the grid's view only).
- **USE TIME** (`HealthEffectsComponent`): the medicine's `medUseTime` or the food's `foodUseTime` to one place at
  most, then the word for "sec" with no space ("3sec").
- **Each health effect** (`effects_health`: Health, Hydration, Energy and the rest, in the template's order), named
  by the factor in the install's words, **coloured** by its value's sign (the row's increasing or decreasing colour,
  "+" or "-" put before the whole value): its parts, " / " between, "Del. Nsec" for a delay over a second, "Dur. Nsec"
  for a duration, and the value's size ("+60" on a water bottle).
- **Each damage effect it takes away** (`effects_damage`), only the kinds the item's replacements name
  (`Item._replacements`: the bleedings as STOPS LIGHT and HEAVY BLEEDINGS, REMOVES FRACTURE, PAIN, CONCUSSION,
  TOXICATION, RADIATION EXPOSURE; a destroyed part and a lethal intoxication not listed): "Del.", "Dur." as above and
  "N HP" for a cost, " / " between; **with none of the three, the name alone** (the Grizzly's REMOVES CONCUSSION). The
  bleedings' words end in a line break in the install, which the row does not show.
- **Each buff of a stimulant** (`SetupStimulatorBuffsAttributes`, the set the template names in the globals'
  `Health.Effects.Stimulator.Buffs`, in its order): named by the buff's kind in the install's words, or for a skill's
  `SKILL "<the skill>"`; its value (`StimulatorBuffSettings.GetStringValue`): "Chance N%" below a whole, "Del. Nsec"
  over a second, then under two hours "Dur. Nsec" and, where it has a figure, the figure in brackets; with no such
  duration the figure alone after a space. **The figure** (`GetVisualBuffValue`, `BuffColoredStringValue`): absolute
  to one place, else times 100 to a whole, **halves to the even** (SJ1's -0.25 is -0.2), "+" before one above
  nought, "%" after one not absolute (" C°" for the body's temperature, the word for kg for the weight limit), in
  84,193,255 where the kind and value are a buff (`IsBuff`) and 196,0,0 where not, whatever its sign; the brackets in
  the row's own colour.
- **The icons** come from four tables by the enum the row's id is of (`StaticIcons.GetAttributeIcon`): the
  attributes', the health effects', the damage effects' and the buffs' (`ItemAttributeSprites`, `HealEffectSprites`,
  `DamageEffectSprites`, `StimulatorBuffSprites`, the buffs' names cut to thirteen letters in the Resource).
- **The grid** as for any compact rows: two columns unless a row does not fit its cell (SJ1's one column), filled down
  each column. A value's colour tags take no room in that measure.

**In TRP** (`trp_sim::item_attributes::figures`'s medicine branch, `Table`, `buff_value`; `trp_data::screen::
figure_icons`; `health_view::info_rows` by the row's table; `probe staticicons`). With `TARKOV UI` the game's window;
without it the app's window lists the same rows a line each, colour tags left out, before the description.
**Compared** with the user's captures (`local\GrizzlyInspect.png`, `Calok Inspecty.png`, `Stimulant Inspect.png`
(SJ1), `Stimulant Inspect2.png` (Propital), SPT 4.1.6): every row's name and value in its order the same
(`medicine_s_figures_are_the_captures`), and in ours the same columns (Propital 5 + 5, SJ1 one, CALOK-B 2 + 1) and
icons (`local\shots\med_*_game_s137.png`); our window has some 30 units more between the rows and the description.
**Not built**: a compact row's hover words (`FullStringValue` in the simple tooltip); food's and drink's colours not
captured (read from the row's own figures); a thing not yet examined ("?" for every value).

## Not built

The comparison, the rows of other kinds of thing (parts, magazines, keys, grenades; medicine: built, above), a
part's dropdowns (`CompatibleWith`, `CompatibleWithArmor`), a dropped part's comparison while it is
dragged over a slot, an unchecked chamber's lock and picture, a revolver's drum's border, an item
view's armour class and durability marks in the slots, the
pictures of built-in inserts (made from no model here), a long name's truncation in a slot (the
game cuts it with an ellipsis), the wish list's lines opened beside its button (the app opens them in
the menu at the mouse), the tooltips' own prefabs (drawn in the simple tooltip's look, at the mouse:
`ESTIMATE`), the cursor's look over a stretching place, a repair buff's colour on a value.
