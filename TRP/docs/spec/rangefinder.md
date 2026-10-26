# The handheld rangefinder (Vortex Ranger 1500)

The user, 2026-10-26: "can we put the range finder somewhere along with some reactive targets far
away?" The game's one handheld rangefinder, taken in hand from a quick slot, aimed through to read
the distance to what it points at. Read 2026-10-26 from the game's code and the install
(`local\rangefinder_research_s138.md`, the probe's outputs `local\rf_*_s138r.txt`). Nothing of it
is recorded: a recorder course would need it bound and aimed at known distances.

## The item

- **One template**: `61605e13ffa6e502ac5e7eef` (`item_ranger_1500`), the only child of the node
  `PortableRangeFinder` (`61605ddea09d851a0a0c1bbc`), itself a special item (`SpecItem`). Its prefab
  is `assets/content/weapons/usable_items/item_ranger_1500/item_ranger_1500_container.bundle`; it is
  1 by 1, 0.218 kg. It goes in the pockets' special slots (their filters list its node).
- **Taken in hand by a quick-slot key** only (`InventoryController.IsAtBindablePlace`: a rangefinder
  lying in the pockets or the vest; `Player.TryProceed` → `ProceedPortableRangeFinder`). The body
  animator holds it as a pistol (`Player.GetWeaponAnimationType`).
- **Aimed, it drains stamina** at `AimRangeFinderDrainRate` (0.55 in the install's globals, against
  1.1 for a weapon), the aim drain's rule otherwise unchanged.

## The hands

- **Its controller** (`PortableRangeFinderController`, a usable item's): the hands are the prefab's
  rig (`item_ranger_1500.generated`) and its animator; the eye is brought to the rig's own
  `aim_camera` bone (no sight on it, so no zeroing and no calibration); the aiming speed 2, with no
  aim sway, no overweight and no ergonomic weight (`ManualSetVariables(2, 0, 0, 0)`); the turn-away
  as a weapon's, 0.5 m. No recoil.
- **Its animator** (`item_ranger_1500`): drawn from `SPAWN` to `OUT TO IDLE` (0.3 s, in hand at
  `WeapIn` 0.9 of it) to `IDLE`; from `IDLE`, `AIMING` while `Fire` is set (its clip at speed 2,
  0.333 s; back through `AIM_TO_IDLE`), `LOOK` on its trigger (1.333 s), `IDLE TO OUT` when not
  `Active` (out at `WeapOut` 0.981 of it), and the inventory's states on `Inventory`.
- **Its keys** (`UsableItemInputTranslator`): the aim key aims and lets go (as the binding presses
  it, held or toggled); the examine key plays `LOOK`, at rest only; fire, reload and the fire mode
  do nothing.
- **Aiming** (`PortableRangeFinderIdling.SetAiming`): the base refuses the aim where the body cannot
  aim (`CanAimInState`) or the turn-away is past its stop; else it sets `Fire` and the aim. Then, in
  first person and **whatever the base did**, the optic and the readout are switched to the aim
  asked (a quirk of the game's, kept). A sprint begun, a state that cannot aim, the inventory and a
  gesture let the aim go. Hidden (`Remove`): `Active` off, the aim, the optic and the readout off.
- **The main camera's field of view does not narrow** as it aims (`OnAimOrPoseChanged` narrows it
  only with a weapon's data, which this has none of): the setting's stands (`PROVISIONAL`, from code).

## The readout (`TacticalRangeFinderController`)

- While aimed (its object on), every 0.5 s, and once as it comes on: one ray from the `optic_camera`
  bone along its forward (the rig's forward), up to 999 m, against the world's doors, terrain,
  solids, characters' hit colliders and corpses (layers 9, 11, 12, 16, 23). Its text: the distance
  clamped to 0 to 999 and written `000.0` (three whole digits, one decimal), or `---.-` where the ray
  meets nothing; kept between rays.
- **Where it is drawn**: into the optic's picture, as the optic camera's screen (a canvas 1024 units
  across), the text's middle 35 units right of the picture's middle and 194 up, in the game's Bender
  font, italic, at size 125, red (1, 0, 0.036), every digit as wide as the font's widest.

## The optic

- **One mode, fixed** (`ScopeCameraData`): the optic camera's field of view 3.2 degrees, near 0.05,
  far 1000; on with the aim, off without. Its lens `ranger_linza` is the scopes' lens shader with
  the mark `rangefinder_reticle` (`optics.md`, "The lens": built). Its magnification is given
  nowhere (about 7x by the field of view, `ESTIMATE`; the real device's is 6x).

## Built (0.143.0)

- **Where it is on the range**: in the grenades' table's box, with the knives (the box of melee
  weapons); picked up, it goes where a pick-up puts it, and is bound to a quick key from the pockets
  or the vest as anything bindable is (hover it in the inventory, press the key).
- **The hands** (`trp_sim::rangefinder::RangefinderHands`; `weapon.rs`, `RigKind::Rangefinder`): its
  prefab's own rig and animator, drawn from a quick key, put away by anything else asked for; the aim
  key's toggle or hold as the binding says; the examine key's `LOOK`; the aim dropped as a weapon's
  sights are (a sprint, a state that cannot aim, a long fall, a vault, the inventory). Its aim speed
  2 with no sway (`Hands::manual_set_variables`, kept over the weight's recomputes), and the main
  camera's field of view kept as set (`Hands::keep_fov_aimed`).
- **The optic**: its prefab's sight part (`trp_data::sight::prefab_part`, the node with
  `ScopePrefabCache`) forced live while its controller has the optic on (`Sights::force_optic`), so
  the quirk shows: the optic picture is drawn whenever the aim was asked, taken or not
  (`HandsRig::optic_seen`).
- **The readout** (`rangefinder_view.rs`): the ray from the optic object placed as the optic camera
  is, along its forward, to 999 m, against the range's boxes, doors and ground, the dummies' hit
  colliders and the poppers; the text a UI node aimed at the optic camera, in the build's Bender
  Italic at 125, red, its box 200 by 50 centred 35 right and 194 up of the 1024 picture's middle.
- **The stamina**: the aim weighs 1 at `AimRangeFinderDrainRate` while it is in hand
  (`Stamina::set_rangefinder`), and the walk is held to the aim's 0.33 as the sights hold it.
- **Not built**: the digits' equal widths (`SetMonospaceText`: Bevy's text has no `<mspace>`; the
  figures move by a pixel or two as they change), the readout with `PIP` off (no optic picture is
  drawn then), the canvas's 1 m cull, its depth at 2 m (`PROVISIONAL` in the research: drawn over
  everything here), the turn-away (not given to this rig), the third person's own rig.

### Compared with

Nothing recorded. Looked at in captures (`local\shots\rf_*_s139.png`): taken from the pockets by its
key, aimed at the lanes (the ground near the horizon, 849.9 m), at the sky (`---.-`) and at the
standing dummy from 3 m beside it (002.5: the device is held some 0.4 m before the eye and the body's
surface is nearer than its middle). The number sits between the reticle picture's own `HCD` and `M`,
which the picture leaves room for: the offset and the size are the prefab's.
