# Backlog

What is missing, feature by feature, as of 0.100.0 (2026-10-21). Two kinds of entry:

- **Reported**: the user's own list (2026-10-21), with what is already known about each (a cause
  found, where to look). These are what the user has seen.
- **From the specs**: what each spec in `spec\` (and `status.md`) says it has not built or only
  approximated; "(approx.)" marks a `PROVISIONAL` or `ESTIMATE`, each also listed in `status.md`.
  Short: the spec has the detail. Lines a later part of the same spec says are built are left out.

The options' overhaul the user asked for is built: three lists, SIMPLE, EXPERIMENTAL and ADVANCED
(0.101.0: the user's commit of 2026-10-21, landed 2026-10-23; the page: `settings.md`, "Features:
what is switched"). What is left of it, `LITE INVENTORY`, is at the end ("The feature options").
`status.md`'s "Missing" list is stale in places (it still lists the right-click menu,
using medicine, the explosion's second part, the frame limit, SSAO and others as missing): to be
tidied when it is next touched.

## The character controller: what is left (2026-10-24, the user: "Give me what's left over to implement for the controller")

Checked against each spec's own lists and the code (several lines below were stale: the aimed
reload's stiffening, aim sway, falls, the goggles' key are built). The user's own four first.

- **Tactical devices**: done (0.120.0, `TACTICAL DEVICES`): flashlights and lasers on the weapon,
  the helmet's head light, their keys in Press and Hold, their modes, the hands' switch, the HUD's
  panel (`TacticalInteractionPanel`); compared with course 7 frame by frame (`tactical-devices.md`).
  Left: the light shaft and the flare (asked: `decisions.md`, open question 1); the weapon's own
  lights and lasers against a recording (recorder 0.13.1's course 7, by hand too: the user's to
  run); the gear's ergonomics penalty (the hip penalty's E); a slung weapon's lights; night vision.
- **Visors and what is worn on the face**: built are the switch, its key `N`, the gestures, the
  hinge (a shield's own too, 0.118.0), the first-person mask (0.118.0) and the icon following
  them. Left: the glass drawn see-through (opaque now; what it needs: body animator spec, "The
  shield's glass"); a visor's hits, cracks and shattering; night and thermal vision (not for now,
  the user, 2026-10-24).
- **Blind fire** (over the top and to the side): done (0.123.0, `blind-fire.md`). Left: the weapon
  while the body carries it in part (its way in, a change of side: up to 5 cm and 7 degrees off); the
  overlap's blind-fire line beside a recording with walls; recorded: a pistol, looking far up, the
  lean's quirk, a jump in it, from outside.
- **Mounting and bipods**: done (0.125.0; `spec\mounting.md`): the search, the mount and its exits, the
  bipod's toggle, the keys, the switch, the weapon's motion on it, its sounds, the skill's ergonomics.
  Left: the tactical reload's stiffening (forward on the bipod and in an aimed reload), the bipod's legs
  and its own sounds (the battle screen's mount icon: done, 0.126.0); a recording of a side mount, a bipod mount and a
  standing one (recorder 0.13.1's course 8 by hand, `blind_mount_s127.md` section 4's M1 to M4).
- **The left shoulder** (`LeftStance`): done (0.127.0, `left-shoulder.md`; a canted sight has no turn of its own in
  it: the game's `LeftStanceRotation` is never read). Left: the weapon's left hand's own gestures and layer, a
  pistol and the wall's drop against a recording, third person looked at.
- **Malfunctions and overheating** (misfires, jams, the fixes; heat, wear), with Troubleshooting: done (0.128.0;
  `malfunctions.md`, "Not built": repair buffs, the voice lines, the shimmer, the glow in thermal vision, the
  pistol's quick draw after one).
- **The weapon's smaller parts**: the hand shake after a fast switch (the pistol's quick draw, a
  double press, done 0.134.0); the idle fidget after five minutes (`TOO IDLE`); the left-hand gesture layer; an
  underbarrel launcher on a rifle; a round loaded by hand, the chambered round taken out, the
  magazine keys (`NextMagazine` and the rest); the muzzle's light.
- **Movement**: steep slopes and sliding, and the sprint's slide: done (0.130.0; `movement.md`, "Slopes
  and sliding"; left: standing on another character, the slide out of a sprint against a recording); the sidestep's, roll's, prone's
  and crawl's refusals at obstacles and edges, with the headroom: done (0.131.0; `movement.md`, "Room to move"; left: against a
  recording, the inventory's vault refusal); the head's half of the push out of walls: done (0.132.0;
  `movement.md`; left: the push against recorded walls, which needs readable meshes); speed
  limits from armour, injuries and being shot: done (0.133.0; `movement.md`, "Speed limits"; injuries'
  were built; left: a swamp's, which the range cannot cause, and a hit's against a recording); stamina
  drained by wounds and a fast switch: done (0.134.0; `stamina.md`, "Wounds and the fast weapon switch":
  the quick draw itself with it, its hand shake left).
- **Health**: stimulators' and food's buffs: done (0.135.0; `health.md`, "Buffs": the HEALTH tab's
  rows of a stimulator left); the hurt voice and the health sounds: done (0.136.0; `health.md`,
  "The voice": its equalisers and reverb left); desaturation and
  tunnel vision: done (0.137.0, with the camera's sharpening, `CAMERA SHARPENING`); concussion from blunt head hits: done (0.138.0;
  `health.md`, "A blunt hit's concussion": nothing shoots the player in the range); what the range cannot cause (toxins, radiation,
  cold: a host's).

Recorded so far (corrected 2026-10-24): blind fire and mounting in part (`courses2\4-blind-fire`,
`4-mount`, with the bolt-action M700), malfunctions and heat (`courses2\5-malfunction`, `5-heat`).
Recorder 0.13.0 was run on 2026-10-24 (`courses6`: courses 0 to 3, 6 to 9; not 5, nor the steps by
hand of 7 and 9): the left shoulder (8) waits to be compared; tactical devices (7) were (0.120.0),
blind fire (8) was (0.123.0), mounting (8) was (0.125.0: no mount was held in it). Recorder 0.13.1 (built 2026-10-25) records the weapon in hand's devices too: its
course 7 to be run again. **The hip penalty** by ergonomics and a combo unit on: done for every
weapon (0.120.0; the regression `after155`).

## Reported 2026-10-25 (one message, after 0.125.0; in the user's order)

- **The mount icon**: done (0.126.0; `mounting.md`, "The battle screen's icon").
- **"There is a mis match on the mounting"**, and 2026-10-26 **"the player's model stretches, and it is very jittery
  when I move the aim"** (the sleeves, in first person): fixed (0.138.1; `mounting.md`, "Compared with"
  `courses7\10-anything`): the arms go with the weapon, the drawing no longer shakes, the pivot is turned by the frame
  before's turn as recorded.
- **The AKM's chambered round put back as PS, again and again**: fixed (0.126.0; `firing.md`).
- **"The magazine in the hands is the wrong one"** (2026-10-26: unloaded from the menu, a new magazine dragged on, the
  reload showing the old one's model): fixed (0.138.2; `firing.md`, "A magazine dragged onto the weapon in hand").
- **The left shoulder**: done (0.127.0). **Durability and the weapon's malfunctions, the barrel red from firing too
  much**: done (0.128.0; `malfunctions.md`; left there: repair buffs, the voice lines, the hot barrel's shimmer).
- **Armour and weapon repair kits on the range**, several of each in boxes under the workbench: done (0.128.0);
  **repairing at the bench**: the `Repair Gear` line, had only with a repair kit carried, opening the inventory,
  done (0.128.0); a kit dropped on an item opening the game's repair window (`decisions.md`): done (0.129.0;
  left: the repair buffs, the dropdown's opening, the armour parts' order by their colliders).
- **The workbench's zone past its table**: cut to the table (0.126.0). **The key on a small white table by the
  door**: done (0.126.0).

## Reported 2026-10-22 (the user's answers by multiple choice; to sort into the sections below at the next landing)

- **No respawn after death** ("The deaths seem to match, but I cannot respawn once dead"): done (0.101.1). The death screen heard the main Enter only, not the keypad's; either begins again now (`health.md`, "Built"). The main Enter began again in every death tried (`findings.md`, 2026-10-23). If it fails for the user, the question is how they died.
- **The crates and tables are silent when opened**: "I'd like to give them the appropriate loot opening sound". Done (2026-10-23): the opening, the squeak and the shutting from each thing's Customs prefab, a door's breach and hit with them, under `WORLD SOUNDS` (`interactions.md`, "A thing's sounds"). Still silent: a search's looting, item by item (`sound.md`), and a thing's handle.
- **The HUD under `TARKOV UI` differs from the user's game's** (what differs, not yet said: open question 0). Compare with a capture of theirs in a raid.
- **The weight: 37 against the capture's 36.6** ("The weight figure matters"): traced (0.102.0). It was the HEALTH tab's temperature, not the weight: the game shows the profile's (37), the app its new character's (36.6); left so ("Leave it, note it", `decisions.md`). The weight is the game's to the gram in every kept recording (`inventory.md`, "Weight"). **Still differing in the weight's figure**: its maximum, 95 in the user's game for their Strength at level 40, 77 in the app, which leaves the skill out: with the skills (`skills.md`, Strength's lift, 0.3 over 50 a level).
- **Start at HIGH**: done (0.102.0). The pictures and the shadows start at HIGH, streaming on, Vulkan the renderer; a saved options file keeps its own (`settings.md`, "The shadows"; `streaming.md`).
- **Skill levels from the options tab**: done (0.103.0), with the skills system: the options' SKILLS page, the `SKILLS` switch, the buffs wherever what they act on is built, a host's levels in play shown and kept (`spec\skills.md`, "The skills system"). **Earning**: not built, and to stay so (the user, 2026-10-23: "we will disable that option and leave it for host games to alter levels on runtime"). **Still to act**, each when what it acts on is built: Throwing's cost (a throw's stamina), the stiff breath of Aim Drills' and the classes' elite, Perception, Attention and Search (searching, examining), Troubleshooting (malfunctions), the vests (armour on the player), Weapon Maintenance (wear), Stress Resistance's berserk, Immunity's poison and its elites (poisoning, stimulants' ill effects), Metabolism's ill effects' time (the mounted ergonomics acts since 0.125.0).
- **A bot first** among the third-person models (question 1): a second character with its own rig and controller.
- **DXC's two DLLs ship with a release** (`manual.md`, "Publishing", at the next release).
- **The profile's skill levels change the sounds, and more** (the user, 2026-10-22: "the audio is actually being modified by my installs levels. For example, covert movement lowers footstep sounds by 49 percent. Other skills affect other things"): the app knows the Covert Movement factor on a footstep and on the kit (`footsteps.md`) but applies a level only where a flag gives one; the recordings were made at the user's profile's levels. So a comparison of the sounds' levels (course 7, `sound.md`) applies the profile's modifiers first, and every skill's effect and its figure at each level is read from the install (the skills' settings in the globals, the profile's levels) when the skills system and its tab are built (question 5: from the options tab). Not before: the list of effects is the game's to give, not memory's. **`spec\skills.md` has it all since 0.102.1**: every skill's buffs a level and at elite from `SkillManager`, where each is read (or that it is read nowhere), how each is earned, and the wiki set beside it. Covert Movement's 1.2% a level on common surfaces is the code's: the user's profile has the skill at level 40 (progress 4,000), 48% by it, against the "49 percent" they read (level 41; not settled: the page as they saw it was not captured).

## Interface (inventory, inspect window, workbench, options)

### Reported

- **The inspect window beside the game's** (the user, 2026-10-24, four captures at 2560 by 1440: the
  SPEAR's stock build and the base AS VAL, in the app and the game: "There are still a few
  discrepencies between item parts", "the 3d models on inspect don't match. Every item in our app
  is very shiny, or has no lighting at all"); then, after 0.112.0: "The cells between the weapons
  seem correct, but they have the incorrect font and are missing icons on certain slots", "Keep in
  mind the inspect lighting" (`findings.md`, 2026-10-24):
  - **The model's light**: done (0.113.0): the pool's own template's two directional lights, black
    ambient and photographic tonemap (`inspect-window.md`, "The model"). Left: the model's material's
    second part (`_SpecColor`, `_Glossness`, `_Cube`), the camera's ambient occlusion.
  - **The cells' font and icons**: done (0.113.0): the empty mounts' pictures (by the slot's
    `Name`), the slot names in their own font (the grafted texts' files); the small marks at the
    lower left were the game's already.
  - **"SPEAR 6.8" cut to "SPEAR 6"**: done (0.113.0: the caption cut at its visible letters). The
    1-unit border inset of `slot_box_drawn` against the game's full 64 stays to check.
  - **The magazine's "~ Full" against the sling's "19/20"**: the game's own rule (a magazine not on
    the character directly is told in words at Mag Drills below 1, `GridItemView.SetItemValue`);
    left out is Mag Drills itself, which the app now has (`skills.rs`): its level over 10, and the
    template's `CheckOverride`.
  - **The AS VAL's "SPP", 313 m/s and 3.13 MOA against "SP-5", 293 and 3.44**, and **"0/20"
    against "?/20"**: the two characters' rounds and magazines, not a fault (the figures follow the
    chambered round: 310 × 1.011 against 290 × 1.011; SPP's `ammoAccr` 10).
- **Pictures missing for some items**: built-in armour inserts and others with no model of their
  own have none (the icon maker draws from a model). The game shows a picture for them.
- **Magazine count in the inspect window**: a magazine in a slot of the parts' panel shows the big
  count of a weapon's slot view ("30/30" over its picture); the game's shows a grid view's small
  count ("0/30" in the user's AKM capture). `ModSlotView.CreateItemViewKernel` makes it with
  `CreateItemView(..., slotView: true)`: check which look that gives a magazine.
- **Wrong slot names and pictures in the inspect window** (MOD_TACTICAL_000 on the SCAR, some
  helmets, MOUNT on the USP45 and others). **Cause found**: the game names a slot by the first
  weapon-part type (`EWeaponModType`, in the enum's order) its id contains (`Slot.Name`:
  `mod_tactical_000` is `mod_tactical`, "TACTICAL" in the locale), and finds its picture by that
  name (`Slots/<Name>`); the app uses the id for both (`inventory_view::info_parts`).
  `trp_sim::edit_build::TYPES` has the enum's names in its order. Check the USP45's MOUNT after
  the fix: it may be a slot the game shows too.
- ~~**Dragging a thing does not look like Tarkov's** (the app's own ghost box).~~ Done 0.104.1: its picture centred on the mouse, `R` turning it, the cells or the slot it would take lit (`inventory.md`, "With the mouse"); its own view's look while held is not yet the game's. The blink on a click (the user, 2026-10-23) is gone with it.
- ~~**The loot icon is still broken** (which icon: to ask the user).~~ Done 0.104.2: the LOOT header's icon and word, drawn at no size (the user, 2026-10-23: "fix the loot icon finally"; taken to be the header's, the only broken mark on the panel). The inspect window's differences from the user's captures (AS VAL, 6B23-1) put right the same day; an unchecked chamber's "?" not built.
- ~~**A backpack's or rig's window (double click) is the app's own**, not Tarkov's `GridWindow`.~~ Done 0.124.0 with `TARKOV UI` (`inventory.md`, "A container's window"), with the interaction panel as the game's, its hand, the loot sense's dot, the game's mouse pointer, and the OVERALL figure no longer white (the user, 2026-10-25: "Let's fix the final UI pieces"). Still missing there: the hand and pin cursors (idle, over and, since 0.124.1, the stretch ones are used); a loose armour's MODDING line and EXAMINE (everything counts as examined); the hover pointer only over the game's buttons drawn from its scenes.
- ~~**The SKILLS tab** is not built (skills are not a system either).~~ Done: the skills system 0.103.0, the SKILLS tab as the game's page 0.104.0 (`skills.md`); left: the MASTERING page, a stimulant's boost shown on a skill's view.
- **A thing's picture flickers for a moment when clicked once**: likely the page drawn afresh on
  the click (a press brings a window to the top; any change to what is carried redraws the
  page), its pictures spawned again behind their spinners for a frame.
- **Turning in place as in Tarkov** (the legs stepping round as the body turns on the spot).

### From the specs

- **Inventory** (`inventory.md`): MAP, TASKS, ACHIEVEMENTS, PRESTIGE pages (dimmed); OVERALL's
  statistics and its OVERALL/PMC/SCAV switch; searching a container (they open searched);
  examining (everything counts as examined); found-in-raid, insured and repair marks, an armour's
  class and durability figure, the mastering slider on views; a weapon's caption with its build's
  name; the description's scrolling; the filters' clicks; `BACK`'s hover; the corner glows; text
  auto-size and overflow (cut with "...") and most rich text; binding by dragging onto the quick
  bar, its selectors, the quickdraw glow; the green and yellow light on places that take a carried
  thing; a stack dropped where part of it fits; the move rules for quest, pinned and locked
  things, carry limits, conflicting parts; a moving thing's blink at its old place; unloading an
  ammo box; loading a magazine through the weapon; the middle button; a slot key's refusals and a
  refused move's notice; a container shut as one walks away; the Escape menu; captions 2 to 3 px
  right and the cells 10 levels dark (approx.); pockets, dogtag and side the Standard BEAR's
  (approx.).
- **Right-click menu**: several lines say "not built"; no sounds; on the ground only `TAKE`.
- **Inspect window** (`inspect-window.md`): the comparison (and while a part is dragged over a
  slot); rows of parts, magazines, keys, grenades (medicine, food and drink: done 0.139.0); a part's "compatible with"
  dropdowns; a weapon's size change and repair buff (the discard limit: done 0.139.0); an unchecked chamber's lock;
  a revolver's drum border; the buttons' light under the mouse; the slot views' armour marks;
  long names cut with "..."; the stretch cursor; the wish list's lines beside its button
  (approx.); the tooltips' own prefabs (approx.); mastering points 0 (approx.); the preview's own
  camera and lights (approx.); never set beside the game's windows.
- **Icons** (`icons.md`): the game's icon shaders (approx.); its shadow rim (approx.); folded
  things on the ground drawn unfolded; parts whose material cannot be read left out; the game's
  own icon cache not reused; never compared with the game's icons.
- **Workbench** (`workbench.md`): saved builds (NEW, OPEN with presets, SAVE, SAVE AS, DELETE; the
  user: "Make them work"); typing a build's name; the part under the
  mouse lit on the model; incompatible parts dimmed with the game's reason; "discard your
  changes?" on shutting; a double click on a part to inspect it; the bench's lights; only its
  nearest detail level (approx.). Done 0.140.0: the characteristics panel (set beside the user's captures) and a pick made safe and quick (the user, 2026-10-26: "make a second click during an update safe").
- **Options and keys** (`settings.md`): binds for left shoulder, magazines (blind fire's since
  0.123.0, mounting's and the bipod's since 0.125.0)
  (next, previous, unload), the chamber's unload (the scope, magnification and zoom keys work
  since 0.107.0, zeroing's since 0.112.0),
  fast draw, lights, voice and phrases, the watch, screenshot, console; Escape outside the
  inventory; typing into a slider's box; several displays; NVIDIA Reflex; the game's FXAA and TAA;
  most of the GAME page; the SOUND page's music, chat, hideout, binaural and voice volumes.

## Gunplay (weapons, firing, sights, ballistics, grenades)

### Reported

- **Sights**: picture-in-picture scopes, their zoom levels, switching between several sights on
  one weapon, canted sights, iron sights folding down under certain scopes. Read and specified
  (0.106.1, `optics.md`, with the user's `PIP` switch and the world's zoom without it); built in
  four steps. Step 1 done (0.107.0): the sights as the game picks them, the switch between them,
  the modes, the zoom, the folding sights; step 2 (0.108.0): `PIP` on, the scope's own picture,
  its reticle, the red dots; step 3 (0.111.0): `PIP` off, the world zoomed, the weapon at its size,
  the clear lens with its reticle, the scope's inside cut away from the eye; step 4 (0.112.0):
  zeroing, the optics and red dots turned to the bullet's path, the M32A1's ranging, the HUD's two
  panels. Then (0.113.0) the aim sway, which the M60E6's aim-in on `courses4` lacked; (0.114.0)
  course 6 set beside the app (`optics.md`, "Compared with"); (0.116.2) the lens under held fire
  (the item below); (0.117.0) step 3's rest: with `PIP` off the lens black until the aim settles,
  the game's soft circle and shadow on the clear glass, a black rim (the user's choices).
- **Weapons with no chamber** (found 2026-10-24, 0.114.0, on course 6): the M60E6 fires none of the
  game's 19 rounds in the replay; it, the M60E4, the RPD, RPDN, PPSh-41, Uzi and AA-12 Gen 1 have
  no chamber in the game's data and fire straight from the magazine, where the simulation gives
  every such weapon a chamber (one round too many in the app; `findings.md`). Done (0.114.1:
  `more-weapons.md`, "A weapon with no chamber"); left, the case at the eject's mark.
- **The scope's zoom ring and the Specter's lever** (the user, 2026-10-24: "When scrolling the the
  zoom levels, depending on the scope, the actual model of the scope changes with teh appropriate
  zoom. See the spear for exactly what I mean, or the vudu scope. The specter flips a little lever
  instead of a smooth animation"): done (0.114.0), the ring of every variable scope with one; the
  Specter's lever was built (its modes' objects). Left: the inspect window's ring, as its prefab.
- **A white outline through a scope in the inspect window** (the user, 2026-10-24: "Check the
  velociraptor, it doesn't render in tarkov"): done (0.114.0), the lens, dot and glass pieces left
  out of the previews (`inspect-window.md`, "The scopes' glass").
- **The user's reports after 0.115.0** (2026-10-24), all done (0.116.0): the SCAR's reload from its
  30-round to a Gen M3 40 "cancels out and does not complete ... any time the model changes, with
  any gun" (`firing.md`, "A build made anew in a reload"); the magazine "blink after every bullet and
  duplicate the spinner" while loading (`inventory.md`, "A magazine's rounds drawn"); "Some magazines
  dont display the ammo inside them" (any, after a reload from empty: the same section); "a bullet in
  the chamber when checking chamber" (`firing.md`, "The round in the chamber"); "the STG should use
  the gen 3 40 round magazine animation" (`custom-weapons.md`, "The magazine's clips"). Then "when
  the reload is handed off, the animations stutters": done (0.116.1). Left: a cylinder's rounds
  drawn; the blink fading the main picture alone.
- **The user's reports after 0.114.1** (2026-10-24), all done (0.115.0): "Some icons aren't loading
  now, like the magazines" (the own picture's lookup by the whole key); "magazines like the 60 round
  or drum mag should change the left hand animations" (the fitted magazine's clips, for the reload
  and the look: the game's hold at rest has none; `firing.md`, "Which magazine"); "give the workbench
  model the same lights as the inspect menu", "our character in the overal screen needs one as well"
  (asked: the inspect window's, `decisions.md`); "when you hover over the differnt options like unload
  or reload the boxes should highlight"; "the scroll bar next to the weapon description should only
  appear when needed, and actually work" (`inspect-window.md`, "The buttons", "The description").
  Left: a `Blocked` button kept lit, a disabled button's reason as a tooltip.
- **The lower right's other panels** (the user, 2026-10-24: "all controls that open that little
  menu"): the magazine's look, the selector's, the fire mode key and the force to automatic are
  built (0.113.0); the chamber's look names nothing in the game. The tactical device's press or
  hold mode (`TacticalInteractionPanel`): done (0.120.0). The watch's
  time (`GameTimePanel`) is not wanted (the user, 2026-10-24: "The watch's time isn't necessary").
- **A canted sight** (the user, 2026-10-23, after 0.107.0: "canted scope is missing the animations"): a sight
  turned aside (an offset red dot) is chosen, but the weapon is not turned onto it as the game
  turns it (`UpdateCurrentScopeRotation`, the sight's `Rotation`): after step 2. Recorded since
  (2026-10-23, `courses4`): the M4A1 aimed through a Belomo PK-06 on a 45 degree mount, the weapon
  turned 45 degrees about its bore. Done (0.110.0): the turn, frame for frame within 0.007
  degrees of the recording (`weapon-handling.md`, "A canted sight"); the left stance's turn not.
- **The PK-06's window** (found 2026-10-23, 0.110.0): it draws as an opaque grey disc with no
  dot, on a canted mount or on top; other red dots draw theirs (`findings.md`). Done (0.110.2):
  its window (`EFT/CollimatorGlass`) read as the front glass and not drawn; its reflections not
  built, as a scope's glass's.
- **The scope's picture: the muzzle flash and the bullets' effects** (the user, 2026-10-23, after
  0.108.0: "The muzzle flash and bullet effects are rendered when scoped in tarkov"): the app's
  flash, smoke, fumes, sparks, haze and the particle systems are drawn for the main camera alone
  (`muzzle_view`, `particles_view`, `explosion_view`: each fed by a system with its main camera's
  figures), so the optic camera's picture lacks them. Done (0.109.0): the jets, fumes, sparks and
  the hits' and blasts' systems in the picture, each stepped once; the gun smoke's trail and the
  heat haze left out, as the game draws them for the main camera alone (`optics.md`, "The optic
  camera"). Looked at in the app, not beside the game's.
- **The scope's lens under a held fire key** (seen 2026-10-23, after 0.109.0): the Specter's lens
  shows a dark top and a pale disc while the M60E6 fires on, the same with every effect off (step
  2's pose under sustained recoil; `local\shots\m60e6_fire_aim_*_s120.png`). Done (0.116.2): the
  eye's place, not the lens. In automatic fire the game keeps the eye on the sight's own line; the
  simulation had it 4 to 6 mm above the Specter's axis through the burst, which slides its shadow
  most of the window. Measured on course 6's held fire, the lens's drawing checked against the
  shader's reading (`optics.md`, "Compared with"; `weapon-handling.md`, "Camera"). Not compared
  with the game's own picture under held fire (no capture).
- **`PIP` only where it is seen** (the user, the same day: "Make sure PIP does not render in third
  person or when not scoped to save in performance"): so built (0.108.0: the optic camera on only
  while aiming through an optic in first person, `optic_view::place_camera`).
- **The muzzle's jet lives too long** (the user, the same day: "The jet stream expands and
  dissipates over a few seconds in the actual game but lives for a long time in ours"): done
  (0.110.1), the gun smoke's trail aged at the game's frame rate (the Renderer's item below).
- **Tactical devices**: flashlights and lasers: done (0.120.0).
- **Blind fire**, with every weapon: done (0.123.0; compared with the MDR and the M60E6).
- **Vaulting has no sounds**: done (2026-10-23), the three moves' marks from the body's states, each surface's elements and the kit's banks as the install has them, under `MOVEMENT SOUNDS` (`footsteps.md`, "The vault's sounds").

### From the specs

- **Firing** (`firing.md`, `weapon-handling.md`): malfunctions and overheating done (0.128.0; left:
  `malfunctions.md`, "Not built"); aim sway done (a heavy weapon's, `weapon-handling.md`);
  mounting and bipods done (0.125.0; the optics' aim planes, eye distance and the choice of sight: done, 0.107.0); the hand
  shake after a fast switch (the quick draw itself done, 0.134.0); the stiffening in a reload with the sights up; the left-hand gesture
  layer; the idle fidget after five minutes; the muzzle's light; an underbarrel launcher's fumes;
  spent cases for multi-chamber weapons, the underbarrel launcher, misfires and jams; loading one
  round by hand (`LOAD ONE`), taking the chambered round out, rechambering; parts fitted through
  the hands; a menu reload with no room for the old magazine; skills' effects beyond the reload
  and draw speeds, fast pistol draw, damaged arms' speeds; never compared: the muzzle effects, the
  cases, the quick reload, the skill speeds, the launcher blasts.
- **Ballistics** (`ballistics.md`): the optic's zeroing; fragments as bullets; a double barrel's
  second shot and a revolver's double action; explosive rounds; tracers; the flyby sound; shot
  sounds by ammunition; the range's materials the app's (approx.); a weapon always new (approx.).
- **More weapons** (`more-weapons.md`, `bolt-action.md`, `pistol.md`, `shotgun.md`): rockets and
  launcher grenades as projectiles, their sight ranges; rounds and cases drawn in cylinders and
  barrels; a revolver's fistful load, quick dump reload and cylinder roll; the Model 700's
  detachable magazines; a pistol's tactical slot and fast draw; a shotgun's fistful, twin and quad
  loading, other tube sizes, shells drawn in hand; signal pistols and mounted guns (no preset);
  most handling compared with nothing.
- **The StG 44** (`custom-weapons.md`): bolt catch, hammer and catches not animated; the selector
  turns in place; the donor's hand shape, port and chamber points; dust cover always open; the
  AKM's round, recoil and ergonomics (approx.).
- **Grenades and knife** (`grenades-and-knife.md`): fragments as bullets (the range's boxes stop
  the blast instead, approx.); blasts damaging armour; collision and bounce sounds; tripwires; the
  grenade selector; smoke, stun and flash grenades' own behaviour; impact grenades; the knife's
  damage and hit sounds; launcher grenades; the blast's occlusion; the throw's release 4 to 12 cm
  off the recorded one.

## Wearables (gear, helmets, armour, clothing)

### Reported

- Done in 0.118.0: **a helmet's visor or attachment did not update its icon** (the key had no
  parts); **the visor's state in first and third person did not follow it** (six shields carry
  their own hinge, never read; no mask in first person).

### From the specs

- The face shield's glass drawn opaque (`Transparent/DepthZwriteDithered`'s checkerboard and the
  camera's `Undithering`: body animator spec); a shield's hits and cracks in its mask; night and
  thermal vision; the head light; a helmet's rail; muffled hearing under headgear (recorded 2026-10-25:
  `courses7\10-anything\anything-212919`, the ZSh-1-2M takes the guns' and ambience's groups to -50 dB
  over 0.7 s; `findings.md`, 0.136.0); a visor's glass
  shattering (`body-hits.md`); armour repair buffs.
- The body seen from outside: hoods, the armband, slung weapons, the torso's mesh swap under
  armour (`body-animator.md`).
- **Outfits** (`outfits.md`): see-through glass (SEVA); the suit's own figures (the PACA's,
  approx.); Tarkov's vests, backpacks and headwear not fitted to the broader body; the other
  game's normal and gloss maps (a rough cloth, approx.).

## Bots and dummies

### Reported

- **Their skills** at level 1 where the game's rules need a level.
- **Hearing their footsteps** (and the rest of their sounds): their steps and kit since 0.106.0 (`footsteps.md`, "A character seen from outside"), and their weapon's landing at death (`sound.md`, "A weapon landing"); still silent: their shots, voices, the hands' sounds; their occlusion and the binaural plug-in not built.
- **Hit effects** on them (blood, the hit's particles and sound).
- **Explosions should affect them** (a grenade's blast reaches the player alone now).

### From the specs

- No real second character (bot or player): the dummies stand idle or walk sideways (the
  user's "later" for their own animations) (`third-person.md`, `body-hits.md`).
- Seen from outside: grenade and knife states, the left-stance layer, stationary weapons (mounted
  ones from code since 0.125.0, never recorded), the feet solver, levels of detail, distance culling, the weapon's third-person
  effects (muzzle, tactical devices); where a character is heard from.
- Hits: debuffs (speed, stamina), the pain voice, blood on walls behind and the bleeding's drops;
  never compared with the game. **The flinch**: none in the single-player game (its code switches
  the hit reaction off for every bot; the recordings show none: `body-hits.md`, "No flinch"); kept
  so by the user (2026-10-26). **The shooter's hit sound**: the hit's own effect's, already played
  (`body_flesh` on the dummy at the recorded volumes).
- Bots' use of the head look (`free-look.md`); dummies drawn at their newest frame (approx.).
- The dummies do not stop the player: the walker passes through the camera (seen 2026-10-26,
  `--start lanes --tap 400:w:450`).

## Renderer (lighting, particles, streaming)

### Reported

- **The world sometimes "blinks" while moving around.** **A bot's weapon blinks** as one walks
  slowly toward it. Done (0.140.1): a material pointed at a raised copy of its picture before the
  copy was on the card was not drawn until it was (`streaming.md`, "How a level changes"; the
  dummies' suits and AKMs, the build screen's receiver). Not reproduced: the inventory's rigs
  drawer "blinking" (the user's picture of 2026-10-26).
- **Things in the inspect window look glossy and highly metallic** (the Salewa, rounds, many
  parts). Where to look: the preview has no ambient light and no reflections (`info_preview.rs`
  and the edit build screen's preview, `AmbientLight` 0), and how the materials' smoothness and
  metalness are taken from the game's.
- **The gun smoke trails ("jet streams") seem to fly forever.** The game ends them: each point's
  spread grows with the square of how far it drifts in a frame, and the trail ends once its
  points have spread past the material's end (`firing.md`, "The gun smoke"). But the drag (0.95)
  is applied once a frame, so a trail's life depends on the frame rate: under a second at 60
  frames a second, several seconds at 144, about a minute at 300 (measured 2026-10-23: 1.3 s,
  6.8 s, 58 s). Done (0.110.1, the user's choice): the trail ages at the game's frame rate
  setting, whatever the app draws at (`firing.md`, "The gun smoke"). (Also as the game's: a
  weapon's first trail starts from the world's origin, eased toward the muzzle.)
- **The ambient occlusion changed with the inventory and with a shot** (the user, 2026-10-23:
  "when opening the inventory it gets darker"): done (0.109.1), always in the game's numbers, the
  inventory's darker look (`ambient-occlusion.md`, "On the range").

### From the specs

- **Lighting** (`lighting.md`, `ambient-occlusion.md`): Tarkov shading's part two (specular
  colour, rim, gloss, cubemap reflections); the reflection cubemaps; bloom, chromatic aberration,
  sharpen, screen-space reflections; MBOIT and engine fog; stars and sun rays; the time of day
  running; the player's Post-FX filter; cloud shadows on DirectX 12; the second AO at HIGH and
  above, colour bleeding; screens over bright lamps too bright; never set beside the game's
  picture. (Now off by default, the user's.)
- **Particles** (`particles.md`): box, circle, edge and mesh shapes; inherited velocity, noise,
  triggers; sub-emitters on birth or death; collision with planes; sheet sprites; ribbon trails;
  wind zones and forces; never compared with the game.
- **Streaming** (`streaming.md`): a map's own budget; SD mode; the DLSS and FSR bias; priorities;
  its timings (approx.).
- **Late pieces**: the upper body seen from outside, dropped things and first-worn items appear
  a moment late (`status.md`).

## Movement, body and health

### From the specs

- **Movement** (`movement.md`, `vaulting.md`): slopes and the sprint's slide done (0.130.0); prone's,
  the crawl's, the sidestep's and the roll's refusals and the headroom done (0.131.0); the
  head's push out of walls done (0.132.0); speed limits from armour and being shot done (0.133.0;
  swamp's not: the range has none); squeezing between
  things and other characters; the vault's refusal with an item being moved (medicine's, the grenade's 0.131.0, blind fire's done).
- **Body** (`body-animator.md`): the feet on uneven ground (tried and reverted); the left-stance
  layer; mirroring and IK passes; vault, climb and interaction states (blind fire's since 0.123.0);
  the face.
- **Stamina** (`stamina.md`): wounds' and fast switches' drains done (0.134.0; the quick draw's hand shake left).
- **Health** (`health.md`): stimulator and food buffs done (0.135.0; the HEALTH tab's rows of one at work left); effects the range cannot cause (toxins,
  radiation, cold); the hurt voice and health sounds done (0.136.0; the speech's equalisers and reverb left); desaturation, tunnel vision done (0.137.0); concussion from
  blunt head hits done (0.138.0; the player is shot by nothing here); fire and minefields; using things in another's container; an item's remaining
  resource on its box; weight softening the weapon's sway; the death screen is the app's.
- **Physics** (`physics.md`): a live round thrown by the bolt; the inventory's throw down;
  sounds of loose things and corpses; the character pushing things (it is not in the engine).

## Sound

### From the specs

- (`sound.md`, `footsteps.md`) Searching and looting sounds; the menu's, the options', thrown
  things', the quick keys' and the death screen's; world sounds besides one's own; the hands'
  `PropIn` and `PropOut`; the supersonic crack; occlusion and indoor clips; two-round bursts;
  overheating's pitch; the bipod's own sounds (the mount's done, 0.125.0); the mixer's sends, equalisers and snapshots; vault and climb
  sounds; the snow layer; room reverb; others hearing the player; never compared in a raid; the
  player's voice's equalisers and reverb (`health.md`, "The voice"); a helmet's cut of what is heard
  (`DeafStrength`: recorded, `courses7-anythingnything-212919`).

## The range and the world

### From the specs

- The range is level ground and upright boxes; a level file carries boxes alone; no targets in
  the lanes; doors, containers and the trunk are the app's shapes; a kick has no broken pieces,
  sound or dust; door leaves do not push the camera out; locks on containers and keycards; a key
  never put in the hand (`interactions.md`, `status.md`).

## Tools and performance

### From the specs

- The first time a weapon is hung costs one 50 to 60 ms frame (not traced); the gear page's model
  not parked whole; wgpu's own render cost not addressed; recorders 0.10.1 and 0.10.2 never run;
  the installer and a fresh clone never tried; fullscreen and borderless never run (`status.md`).

## The feature options (built 0.101.0; Lite Inventory not built)

The user (2026-10-21): "I want to overhaul the feature list. My vision is to have 2 Main
categories" (three named): **Simple**, "very basic and easy to understand features" (Tarkov UI);
**Experimental**, "features that were not originally in Tarkov but may help with passthrough or
QOL" (Lite Inventory); **Advanced**, "whatever systems we have in place that are not so
accessible, but may be important in order to make passthrough play easier. Like our light
system". "I need help with the categories but we should definitely make them from scratch."

**Built in the user's commit of 2026-10-21, landed as 0.101.0** (2026-10-23): the proposal that
stood here, layered (a SIMPLE group turns its parts together, ADVANCED lists them one by one).
The lists, every switch's place, its name and its key: `settings.md`, "Features: what is
switched", their one home. Where the build differs from the proposal: the effects' group is
`VISUAL EFFECTS` (proposed WEAPON EFFECTS); `HEALTH SYSTEM` kept its name; `HALL LAMPS` kept, as
`LEVEL LAMPS`, for a level file's lamps; `AMBIENT OCCLUSION` sits with the lighting; the old
`STAMINA` is `STAMINA DRAIN`, its key `stamina` now the group's. **Not taken**: the proposal's
"EXPERIMENTAL off unless the user chooses": each kept its default, all on (the user's to say if
wanted).

**Lite Inventory** (the user's design, 2026-10-21; not to be built until told; since 0.101.0 its
row stands last under EXPERIMENTAL, a dash for its switch, saying what it will be): "Inventory like
the STALKER Series. Disables per-item storage slots including special items. Instead, the right
side is an expanding inventory that grows the more you put in. On the left side, we have whatever
container is opened to loot whether it be a crate or body etc, dropped backpacks and rigs should
never have items inside, and can only be picked up. Rigs increase available quick slots, starting
with only 1, 2, 3; you would need a rig for 4 and beyond. The bigger the rig used to be (amount of
cells inside), the more quick slots it provides, up to the full quickbar amount. Backpack increase
max possible weight." To settle before building: how many quick slots a rig of so many cells
gives; how much weight a backpack adds; what happens to what is in a rig or backpack as it is
turned on; whether the secured container and the special slots go too.
