# Patch notes

## TRP 0.143.1 (2026-10-26)

- Fixed: the armour and weapon repair kits showed as plain white boxes, in the inventory and on the ground. Tarkov gives them an invisible box that only casts their shadow; it is now drawn that way. Pictures made before are drawn again once. 

## TRP 0.143.0 (2026-10-26)

- The Vortex Ranger 1500 rangefinder, in the grenades' table's box with the knives. Put it in your pockets or rig, bind it to a quick key, take it in hand and aim through it: the red readout shows the distance to what it points at, measured every half second, `---.-` when nothing is in reach (999 m). It drains stamina at Tarkov's own rate while aimed, and slows the walk as aiming does.
- A rangefinder's picture in the inventory, drawn from its own model.

## TRP 0.142.0 (2026-10-26)

- Reactive targets: Tarkov's steel poppers from the hideout shooting range, in rows of three down the lanes at 25, 50, 100, 200 and 300 m, each row with a white distance sign. Hit the round face and the plate falls back, standing up again 3 seconds later; hit its neck or edge and it only rocks, with Tarkov's own sounds. A new switch, REACTIVE TARGETS, turns them off.
- The range is laid out as you drew it: the stairs and ledge mirrored, with the lower platform on the right and the storage against the stairs; the knee-high wall longer, with every table along it; the dummies nearer; the cover to the right; the surfaces sampler beside the yard.

## TRP 0.141.0 (2026-10-26)

- The F1 debug panel has commands now, clicked with the mouse while it is open: hunger, thirst and stamina up, down, empty or full; heal everything, hurt the next part, die; the weapon in hand's durability up or down, heat it or cool it, fill its magazine, clear a malfunction or give its next shot one; dress the dummies as you. Each shows its value beside it.
- The list of keys is gone from the panel (the options' CONTROLS tab has them), and F7, F8 and F9 are no longer keys: hurt, die and dressing the dummies are the panel's buttons.

## TRP 0.140.1 (2026-10-26)

- Models no longer flash out for a frame as you walk up to them: the dummies' clothes and weapons, and parts of the weapon on the build screen, are drawn the whole time while their textures sharpen.
- A suit chosen on the OVERALL tab now goes on once all of it is ready, so the body is never drawn without one in between.
- Walking costs fewer frames than before: the character's animations are worked out more cheaply, with the same result.

## TRP 0.140.0 (2026-10-26)

- The workbench's build screen now shows the weapon's characteristics at the bottom left, as in Tarkov: durability, weight, ergonomics, accuracy, recoil and the rest. Rest the mouse on a part in a dropdown to see how it would change them; the side tab folds the panel away.
- Picking a part on the build screen is now quicker, and clicking another slot while the new model loads no longer goes wrong: as in Tarkov, the slots come back once the model is ready.

## TRP 0.139.0 (2026-10-26)

- Inspecting medicine, food or drink now shows what it does, as in Tarkov: its HP resource, its use time, what it cures and its health effects, and a stimulant's buffs and side effects with their delays and durations. It shows in both the Tarkov-style window and the app's own.
- Items with a discard limit now show "Cannot be discarded in raid" at the top of their window.

## TRP 0.138.2 (2026-10-26)

- Dragging a magazine onto the weapon in your hands now shows that magazine's model through the whole reload, also after unloading the weapon from its menu, as in Tarkov.

## TRP 0.138.1 (2026-10-26)

- Mounting no longer stretches your sleeves: your arms and shoulders now travel with the weapon when it rests on cover, as in Tarkov.
- Turning while mounted no longer makes the weapon shake: the weapon, the view and the cover stay together, and the weapon leans off its rest by each frame's turn the way Tarkov's does.

## TRP 0.138.0 (2026-10-26)

- A round your helmet stops can still concuss you, as in Tarkov: the harder the hit the helmet takes, the longer the blur and ringing.

## TRP 0.137.0 (2026-10-26)

- Tarkov's camera sharpening is in (a new switch, CAMERA SHARPENING, on with CUSTOMS LIGHT): the picture sharpened at your Tarkov's own Sharpen setting and its colours faded a little by the weather, as on Customs.
- Being exhausted greys the view, and on painkillers grey closes in from the edges and opens again as they wear off, on Tarkov's timings.
- A stimulant's tunnel vision darkens the edges of the view.

## TRP 0.136.0 (2026-10-26)

- Your character now has Tarkov's voice, your profile's own: a grunt when hit or hurt by a fall (quiet on painkillers unless the hit is over 4), a grunt now and then while bleeding, a death cry (none for a shot to the head), heavy breathing when out of breath or badly hurt, and a calm breath behind a face shield that covers the jaw.
- A broken bone cracks as it breaks.
- A new start, `--start open`, with a long stretch of open floor ahead.

## TRP 0.135.0 (2026-10-25)

- Stimulants, and the food and drink that carry buffs, now work as in Tarkov: Propital heals you over time and raises your Health, Vitality and Metabolism skills, SJ6 raises your stamina, M.U.L.E. lets you carry half as much again, Zagustin stops bleeding, each on the game's timings and followed by its side effects (shaking hands, pain, thirst, hunger, lost health). Taking the same one again makes it last longer.
- A skill raised by a stimulant counts at its raised level everywhere it acts, and can reach elite while it lasts.

## TRP 0.134.0 (2026-10-25)

- A hit by a weapon now burns your stamina, as in Tarkov: the round's stamina damage times the hit comes off your legs at once.
- The quick draw: press 1 twice to put away the gun in your hands and draw your pistol faster; each half costs your arms 15 stamina (less with the weapon's skill at 50). Pressing 1 once now waits a moment for the second press, as in Tarkov. 

## TRP 0.133.0 (2026-10-25)

- Armour now slows you as in Tarkov: the armour pieces and plates you wear, and your backpack, take their speed penalty off your top speed (less the Light and Heavy Vests skills' share), and the speed display shows the limit.
- Being hit by a weapon slows you for two thirds of a second, more the more stamina the hit burns, and a hit in the leg ends a sprint. 

## TRP 0.132.0 (2026-10-25)

- Your head is now kept out of walls as in Tarkov: leaning into a wall pushes you away from it instead of putting your head through it, and the same holds looking down over something or lying down.
- A lying body will not turn its head into a wall, and a mounted weapon's turn now stops where Tarkov's does, by the head rather than the whole body. 

## TRP 0.131.0 (2026-10-25)

- Tarkov's checks for room are in: a step aside into a wall or over an edge does nothing; lying down needs a body's length and ground no steeper than 30 degrees; getting up under a low ceiling stays down, or comes up crouched where there is room only for that; and no stance is raised into a ceiling.
- A roll beside a wall is refused, a crawl stops short of an edge, a lying body will not turn over an edge, and going down against a wall pushes you off it.
- With medicine or food in hand, or a grenade cooked, a wall that needs a hand to vault is not vaulted. 

## TRP 0.130.0 (2026-10-25)

- Ground too steep to stand on (steeper than about 49 degrees) now slides you down it, faster the steeper it is, as in Tarkov; walking down a slope keeps your feet on it instead of stepping off into the air. (The test range itself is level.)
- Crouching while sprinting is Tarkov's slide: you drop into a full crouch at once and come out of the sprint low. 

## TRP 0.129.0 (2026-10-25)

- Repairing at the workbench: with REPAIR GEAR's inventory open, drop a repair kit on a gun (or armour) and Tarkov's own repair window opens with that kit: the gun's model, the kit as the repairer with its points, Tarkov's quality, repairability and enhancement lines, the condition bar with its slider and its button to the top, the points to pay, and the warning of how much greatest durability the repair will take.
- REPAIR repairs as your SPT install's server does: the durability goes up, the greatest durability drops by a random share of the kit's figures, and the kit's points go down (an empty kit is used up). Tarkov's sound and its notice "Item successfully repaired to ..." follow, and a gun in your hands takes its new durability.
- A repair kit's picture in the inventory shows its points left, as in Tarkov. 

## TRP 0.128.0 (2026-10-25)

- Guns now wear with every shot, as in Tarkov: by their parts and their round, faster when hot, and a gun run very hot loses some of its greatest durability. Weapon Maintenance slows it. The inventory shows each gun's durability as it falls.
- Guns now malfunction, with Tarkov's chances from their durability, their round, their magazine and their heat: misfires, failures to eject, failures to feed and jammed bolts. The gun stops with Tarkov's own animation; the trigger then only clicks. L looks at the gun to learn what went wrong, and Shift+T clears it with Tarkov's animation, faster with Troubleshooting and slower with a broken arm. A reload, the magazine's look and the inventory refuse what Tarkov refuses ("You must fix malfunctions first"); a failure to feed throws its round onto the ground when cleared.
- Tarkov's messages at the lower right ("Malfunction: Failure to eject", "Malfunction! Examine your weapon - L", "Fix malfunction: ... - T + LShift"), with their sound, and Tarkov's red, yellow and green glow in the lower right corner. A gun with a known malfunction shows red in the inventory.
- Guns now heat with every shot and cool after, as in Tarkov. A hot gun wears faster, spreads its shots wider, malfunctions more, may change its rate of fire, can hold its bolt back, and may fire a round by itself. Its barrel glows red.
- Shots now spread wider from a worn barrel, as in Tarkov.
- Three new switches among the options' SIMPLE features: DURABILITY, MALFUNCTIONS, OVERHEATING.
- The range has two boxes under the workbench, one with six body armour repair kits and one with six weapon repair kits. The workbench's top is now a thin slab on four legs, so what is under it can be seen and reached.
- The workbench's menu has a REPAIR GEAR line, pressable with a repair kit in your inventory, which opens the inventory for the repair. Tarkov's repair window, opened by dropping the kit on what to repair, comes next. 

## TRP 0.127.0 (2026-10-25)

- The left shoulder: the side mouse button (Mouse4) carries the gun on your left shoulder, as in Tarkov, and again puts it back on the right. The body plays Tarkov's own move, the gun goes over to the left of the view, and aiming brings the sights in front of the eye with the left arm in view. Set beside a recording of Tarkov, the move matches frame for frame, the gun within 4 mm and the camera within 2 mm.
- As in Tarkov it switches only with the gun at rest, not lying down, in blind fire or mounted. Sprinting, jumping, lying down, vaulting something high, a reload, a look at the gun or the inventory take the gun back to the right for as long as they last, and it comes back after (at once when a sprint ends); a mount turns it off. Pushed into a wall, the gun goes back to the right until there is room again on the left.
- Blind fire does not start while the left shoulder is on, as in Tarkov.
- A new switch, LEFT SHOULDER, among the options' SIMPLE features. 

## TRP 0.126.0 (2026-10-25)

- With TARKOV UI on, the stance panel at the lower left shows Tarkov's mount icon while the gun is mounted, with an arrow for the side it rests on, and the red icon flashes when V finds nothing to rest on.
- Fixed: dragging the chambered round out of a weapon in hand (an AKM with its magazine out) put a PS round back in its place, again and again. The hands now take the round out with Tarkov's own animation, and the chamber stays empty. With a magazine in, the round stays, as in Tarkov.
- The workbench's menu shows only while standing in front of its table, no longer beside it.
- The yard door's key now lies on a small white table by the doorway. 

## TRP 0.125.0 (2026-10-25)

- Weapon mounting: V rests the gun on a sill or the top of a wall in front of you (or the edge of a wall beside you), as in Tarkov. The character steps up to it, the gun settles on it, and looking around then leans you and raises or lowers you with the look, as far as the cover allows; recoil and sway are lower on it.
- Moving, leaning, jumping, crouching, reloading or V again takes the gun off; lying down, vaulting or opening the inventory takes it off at once.
- Ctrl+V puts a gun's bipod out or folds it (the M60E6's, for one), with the body's own movement; lying down with it out, V rests the gun on it.
- The kit rattles as you mount and leave, and the gun knocks on the cover as it settles.
- A new switch, MOUNTING, among the options' SIMPLE features. 

## TRP 0.124.1 (2026-10-25)

- With TARKOV UI on, the mouse pointer turns into Tarkov's stretch arrows over the edges and corners of an item's inspect window, and keeps them while you stretch it.
- After clicking one of Tarkov's buttons the pointer goes back to the plain arrow until you move off the button, as in Tarkov. 

## TRP 0.124.0 (2026-10-25)

- With TARKOV UI on, looking at something now shows Tarkov's own menu: its lines in Tarkov's black box above the middle of the screen, a loose item's name under it, and Tarkov's hand pointer. Set beside pictures of the game, it matches to the pixel.
- Tarkov's white dot shows when you look just past loose loot on the ground (the loot sense). Higher Perception makes it easier to catch, and at elite it finds loot all round you.
- A loose backpack or rig on the ground can be searched (SEARCH). An item with no room for it says "No space", as in Tarkov.
- Double-clicking a backpack or a rig opens Tarkov's own container window: its frame, its caption with the item's name, its red cross and the container's own grid layout. It opens where Tarkov opens one, and picking, dragging and closing work as in Tarkov. It also closes when its container is moved.
- The mouse pointer is Tarkov's own, and changes over Tarkov's buttons.
- A door's menu now shows only what can be done (OPEN or UNLOCK, and BREACH): Tarkov's three lines that can never be chosen (BANG & CLEAR, FLASH & CLEAR, MOVE IN) are left out.
- Fixed: the character in the OVERALL tab was drawn all white. 

## TRP 0.123.0 (2026-10-25)

- Blind fire works: hold Alt+W to hold the gun up over cover, Alt+S to hold it out to the right, and let go to bring it back. Switching from one to the other mid-way goes straight across, as in Tarkov.
- As in Tarkov, it starts only standing still with the gun at rest: not while walking, in a shot, a burst, a reload or a jump. It drops the sights, and until you let go you cannot aim, change the sight or fire mode, check the gun, reload, walk or vault. Firing is allowed.
- The body plays Tarkov's own blind fire animations, crouched or standing, and the camera tilts up or to the side by Tarkov's figures. Seen from outside (F5) the gun goes up over the head.
- The camera now sits where Tarkov's does while blind firing; it was up to 12 cm off.
- A wall just in front is cleared by the gun held over it.
- A BLIND FIRE switch under SIMPLE in the options' features (restart required).

## TRP 0.122.0 (2026-10-25)

- A DEFAULT button beside the TIME OF DAY slider puts the range back at 09:30.
- Every run now starts at 09:30. The slider's hour is for that run only and is no longer saved.
- With CUSTOMS LIGHT on, the slider is replaced by a note, since Customs' light is set when the app starts.

## TRP 0.121.0 (2026-10-25)

- A TIME OF DAY slider on the options' FEATURES page sets the range's hour. With CUSTOMS LIGHT off the range's sun, ambient and sky change at once, following Customs' day and night, and the night is dark. With CUSTOMS LIGHT on the hour is taken at the next start.
- Weapon lights are now bright enough to see with CUSTOMS LIGHT off. They were about 800 times too dim.

## TRP 0.120.0 (2026-10-25)

- Tactical devices work: T switches every light and laser on the weapon, Ctrl+T steps their modes, and T+B switches T between Press and Hold, with Tarkov's "Press"/"Hold" panel at the lower right.
- A light on the helmet works with H and Ctrl+H: the hand goes up to it and the light switches 0.8 s later, as in Tarkov.
- The hands play the switch's animation and its click, frame for frame as Tarkov's.
- Weapon lights light the range with Tarkov's cone and picture, and cast shadows (but not of your own gun); lasers draw their thin beam and the dot where they hit. Infrared devices show nothing without night vision.
- Every weapon's wander from the hip now goes by its ergonomics, as in Tarkov, and a combo unit switched on (a light or laser unit) narrows it.
- A new TACTICAL DEVICES switch under SIMPLE in the options' features (restart required).

## TRP 0.119.1 (2026-10-24)

- The 5.56x45 M995 round has its inventory picture again, and is drawn when it lies on the ground. Its model was saved switched off, which the game ignores and the app did not. 

## TRP 0.119.0 (2026-10-24)

- Each weapon table's magazine box now holds one of every magazine model its weapons can take, each loaded full, with no duplicates (the rifles' has 79).
- A new olive ammunition box beside it holds a full stack of every round those weapons and magazines take, grouped by calibre (the rifles' has 72). 

## TRP 0.118.1 (2026-10-24)

- No change to the app. The recorder that compares it with the game is ready for its next run: tactical devices, blind fire, mounting, the left shoulder, the face shield, and every sound, animation event and bone the game plays, with pictures of the game's own screen. 

## TRP 0.118.0 (2026-10-24)

- A face shield worn down now shows over your view as in the game: most shields (the ZSh-1-2M's, the FAST's) a light haze of smudges with the edges of the view blurred; the Altyn's, the Rys-T's and the welding masks a dark frame round a rounded window; the Maska's a narrow slit. It comes down from the top as your hand pulls the shield down, and goes as it lifts, and sways a little as you walk. A flashbang's flash washes the smudges out. It can be turned off with FACE SHIELD MASK, under VISUAL EFFECTS in the options' features (restart required).
- A helmet's inventory picture now shows its face shield, and shows it up or down as it is: the picture changes when you fit or remove the shield and when you flip it.
- The face shields that hinge by themselves (the Ops-Core FAST's visor and handgun shield, the EXFIL's, the Kiver-M's, the Vulkan-5's and the TOR-2's) now flip up and down on the body as the others do; before, they never moved. 

## TRP 0.117.0 (2026-10-24)

- With PIP off, a scope's glass now stays black while the sights come up and turns clear once the aim has settled, so the scope's insides no longer show through it during the aim. At the hip it is black, as with PIP on.
- With PIP off, the clear glass now darkens toward its edge as the game's scope picture does: its soft circle, and its shadow, which slides as the eye moves. A thin black rim hides the gaps that showed round the glass's edge.

## TRP 0.116.2 (2026-10-24)

- Holding the trigger with a scope up no longer darkens the top of the lens and leaves a pale disc below it: in automatic fire the eye now stays on the scope's line as the muzzle climbs, as in the game, so the scope's shadow only shifts a little during a burst.
- A shot's kick now starts on the frame after the shot, as in the game: the weapon's own roll and the camera's steadiness carry on for that one frame.

## TRP 0.116.1 (2026-10-24)

- A reload that changes the weapon's model no longer stutters as the new magazine comes in: the animation and the weapon's sway, breathing and lean carry straight on, as in the game. 

## TRP 0.116.0 (2026-10-24)

- A reload that changes the weapon's model (a magazine of another kind, such as the SCAR's 30-round for a Gen M3 40) now plays out to its end: the new magazine appears in the hand as the old one leaves, and the animation carries on.
- A magazine put in by a reload shows its own rounds; a reload from an empty magazine had left the new one looking empty.
- A round sits in the chamber: you see it when the action opens, as in a chamber check.
- Loading or unloading a magazine no longer makes its picture blink with each round: it changes when the game's does (a new kind of round on top, the last round, an empty magazine), and the loading ring no longer flickers.
- The StG 44's magazine uses the SCAR's 40-round reload, so the hand holds its long magazine instead of passing through it. 

## TRP 0.115.0 (2026-10-24)

- Magazines' pictures in the inventory load again (a magazine drawn with its rounds had stopped being made).
- A big magazine plays its own reload and magazine check: an AK's drum, an M4's 60- or 100-round, and the rest, as in the game. The left hand's hold at rest stays the same, as it does in the game.
- In the inspect window, the buttons (Reload, Unload, Discard…) light up under the mouse in the game's colours.
- The inspect window's description scrolls with the wheel, the bar's handle or a click on the bar, and its scroll bar shows only when the text doesn't fit.
- Anything of a window lying over the quick-use bar now takes the mouse, as it is drawn on top.
- The workbench's weapon (the edit build screen) and the character on the OVERALL tab are lit as the inspect window's model is: two soft lights and the same tonemap, with no hot spots or black patches.

## TRP 0.114.1 (2026-10-24)

- The M60s, the RPD and RPDN, the PPSh-41, the Uzi and the AA-12s have no chamber, as in the game: they fire straight from the magazine and hold its rounds and no more (one fewer than before), a reload chambers nothing, and you can't load a round into a chamber they don't have.
- Weapons like these keep whether they are armed, so the M60 arms itself on the draw only when it needs to.

## TRP 0.114.0 (2026-10-24)

- A variable scope's zoom ring now turns as you zoom, as in the game: the SPEAR's Tango6T, the VUDU and the thirteen other scopes with a ring, by the wheel or the magnification key. Two scopes that rest at their narrow end in the model (the March 3-24 and the S&B 3-12) now start at the zoom they hold.
- The inspect window no longer shows a white outline or a coloured disc through a scope or red dot (the Velociraptor's RMR, the SPEAR's Tango): the lens and glass pieces are left out there, as the game draws nothing for them in that window. On the ground the red dots' glass and dot are left out too.

## TRP 0.113.0 (2026-10-24)

- The panel at the screen's lower right now also shows what the game shows there: a look at the magazine (its count in words by your Mag Drills, with the top round's name under it), a look at the selector, the fire mode key and the double-tap to full auto (the mode's name). A look into the chamber shows nothing there, as in the game.
- Heavy weapons sway as their sights come up, as in the game: the M60E6 swings off and settles as you aim; rifles don't.
- The inspect window's model is lit as the game lights it: two soft lights, no hot spots or black patches, the game's tonemap.
- The inspect window's parts: empty mount slots show their picture, slot names are in the game's font, and "SPEAR 6.8" is no longer cut short.
- Magazines become known only when a look at them actually begins.
- For recording: the recorder (0.12.0) has a new course (numpad 6) for zeroing, the panel's messages and held aimed fire.

## TRP 0.112.0 (2026-10-24)

- Scopes zero now: while aiming, Page Up and Page Down step the scope through its distances (the SPEAR's Tango 50 to 300 m, the G28's S&B 50 to 450 m, a T1 25 to 100 m), as the game binds them. Each scope keeps its own distance. The scope's picture, or a red dot's dot, then points where the bullet lands at that distance.
- With `PIP` off the reticle moves on the glass instead, to the same place.
- The M32A1 ranges its grenades: Page Up tilts its reflex sight and lifts the barrel for the distance, as in the game.
- The game's two panels at the screen's lower right: the zeroing distance as the sights come up or the distance changes, and the magnification ("x4", "x6") as you change sight or zoom to an end. They show with `TARKOV UI` on or off.
- Aiming through a scope no longer squeezes the shot and the recoil for a wide field of view; the game stops doing that once a scope is up.

## TRP 0.111.0 (2026-10-23)

- Scopes work with `PIP` off now (SIMPLE options): aiming through a scope zooms the whole view by the scope's magnification, as Anomaly's and GAMMA's 3D scopes do, while the weapon and the scope keep their size. The lens is clear glass with its reticle, which grows with a variable scope's zoom as it does with `PIP` on. The zoom comes and goes as the sights come up and go down.
- With `PIP` off you look straight through the scope: the scope's insides, its mechanism and the barrel ahead are left out of the view through the glass.
- Known: the scope's insides show for a moment while the sights come up, and the lens has no dark rim round it as it has with `PIP` on.

## TRP 0.110.2 (2026-10-23)

- The Belomo PK-06 red dot is see-through now, with its dot: its window used to draw as a solid grey disc that hid the dot. Other red dots with the same kind of window are fixed too. 

## TRP 0.110.1 (2026-10-23)

- The gun smoke's trail now spreads and fades over a few seconds, as in your Tarkov, whatever frame rate TRP runs at. It ages at the game's own frame-rate setting (120 on your install) and is still drawn every frame; before, at high frame rates with the limit off, it hung in the air for a minute or more. 

## TRP 0.110.0 (2026-10-23)

- Canted sights now work as in Tarkov: aiming through a red dot on a 45 degree offset mount rolls the weapon over onto it, so the sight stands upright before your eye, and lowering the sights rolls it back. Change sights with the usual key to go between the scope and the canted dot. Set beside your M4A1 recordings, the roll follows the game's frame for frame.
- Bringing the sights up now starts on the very frame you press the key, and the camera's easing keeps the game's own timing, so aiming in and out sits a little closer to Tarkov's at every frame rate.
- Known: the Belomo PK-06's window draws as a solid disc with no dot (other red dots are fine); on the list.

## TRP 0.109.1 (2026-10-23)

- Ambient occlusion no longer changes when you open the inventory or fire: it is now always drawn as Tarkov draws it, which is the slightly darker look you saw with the inventory open. Corners and the ground under things are a little darker than before.

## TRP 0.109.0 (2026-10-23)

- A scope's picture now shows the world's effects, as Tarkov's does: the muzzle flash, its fumes and sparks, a grenade's blast and what a bullet's hit throws up (dust, fragments, sparks), magnified with the scope. The gun smoke's trail and the heat haze stay out of it, as in the game, where only your own eye's camera draws them.
- A grenade's blast or a hit's dust seen with no flash in the air (with the lighting switches off) is now blended the way the flash already was, as Tarkov blends them.

## TRP 0.108.1 (2026-10-23)

- Nothing in the app changes. The Specter's picture was set beside your capture of Tarkov's at the same size: the scope and its reticle sit in the same place at the same size. Your three new recordings (the M4A1 with its canted red dot, the M60E6, the AS VAL with its 2x) are kept for what comes next, and your notes are on the list: the muzzle flash and the bullets' effects in the scope's picture, and the muzzle's jet that lives too long. 

## TRP 0.108.0 (2026-10-23)

- Scopes now show a picture: a second camera inside the scope draws the world at the scope's magnification, and the lens shows it as Tarkov's does, with the dark rim that slides as your eye moves off the lens's axis and the slight colour fringing at its edge.
- Each scope's reticle is drawn: printed on the lens (the G36's 3x, the Specter's, the Burris's), or, on the SPEAR's Tango6T, the reticle that grows as you zoom in.
- A lens you are not looking through goes dark. The Specter's 4x and 1x each have their own lens.
- Red dots draw their dot (the T1, the G36's red dot).
- A new switch, PIP, on the options' SIMPLE list (on). Off, no second camera is made and the lens stays blank for now; the world zoomed instead comes next. 

## TRP 0.107.0 (2026-10-23)

- Weapons with scopes and red dots now aim through them as Tarkov does: the SPEAR through its Tango6T, the G36 through its carry handle's 3x, the M60E6 through its Specter. Before, the G36 aimed down its own body and the M60E6 over its rear sight.
- Aiming through a scope narrows the view to 35 degrees; over iron sights and red dots it stays 15 below your field of view, as before.
- Right mouse with Left Ctrl switches to the weapon's next sight (the G36's 3x, the backup above it, the red dot on top). Right mouse with Left Alt changes a scope's mode (the Specter's 4x and 1x) and snaps a variable scope from one end of its zoom to the other. Left Alt with the wheel zooms a variable scope, by the options' SCOPE ZOOM ADJUSTMENT SENSITIVITY.
- The mouse's aiming sensitivity follows the sight: lower through a 4x, and through a variable scope by how far it is zoomed.
- A backup sight that folds lies flat under a scope (the M60E6's rear sight).
- The scope's lens is still blank: the picture in it comes next. 

## TRP 0.106.1 (2026-10-23)

- Nothing in the app changes yet. Scopes are written down from the game: how a weapon's sights are chosen and switched, how a scope zooms and changes mode, the second picture in its lens, red dots and holographic sights, and the aiming sensitivity through each. Coming, as the user chose: a PIP switch on the SIMPLE list; with it off, aiming through a scope zooms the world behind while the weapon and scope keep their size.

## TRP 0.106.0 (2026-10-23)

- Loading spinners turn again in every box, small ones too (a medical case's contents with no pictures made yet stood still since 0.104.2), and still keep their shape at a scrolled panel's edge.
- A weapon now sounds as it lands, as in Tarkov: put down from the inventory, or let go by a dying dummy or by you. The sound is the surface's and the weapon's (pistol, submachine gun, rifle), louder the harder it falls. A death is heard as the weapon hits the body, then the ground.
- Taking a thing up off the ground with F plays its pickup sound.
- The walking dummy's footsteps and kit are heard from where it is, as Tarkov plays another character's, fading by the game's own curve over up to 75 m.
- These sounds fade with distance by the game's own curves; the other sounds of the world (doors, crates, hits) are unchanged for now.
- The landing and the pickup come under WORLD SOUNDS, the dummy's steps under MOVEMENT SOUNDS.

## TRP 0.105.0 (2026-10-23)

- The crates, cases, boxes, drawers and the dead Scav now sound as Tarkov's do: each plays the clips its Tarkov prefab names as its lid or drawer begins to move (the wooden crate's lid, a drawer's squeak, a jacket on the body), at Tarkov's volume, and its shutting sound as the lid comes down.
- The doors play their opening as they begin to swing and their closing as they come to; a kick plays the door's breach, a failed kick its thud.
- Vaulting and climbing now sound: the jump-off, the grab, the slide and the landing from the surfaces' own banks, with the kit's rattle at the grab and the landing, each at its mark of the move and at a volume by the obstacle's height.
- A new switch, WORLD SOUNDS, under SOUNDS on the ADVANCED list, for the things' sounds (restart required); vaulting's sounds come with MOVEMENT SOUNDS.

## TRP 0.104.2 (2026-10-23)

- The inspect window now matches Tarkov's: armour inserts show their pictures, their durability ("52/52") and their small marks; slot names read as in Tarkov ("MOUNT", not "MOD_MOUNT_004") and are cut at the end when too long ("FRONT PL"); a part's magazine count is written small, as in Tarkov.
- Armour now shows its durability on its box (e.g. 206/206 on body armour).
- The loot panel's LOOT header shows its icon and its word properly.
- When dragging, the lit cells no longer show past the bottom of a scrolled grid.
- Loading spinners at the edge of a scrolled panel no longer stretch out of shape.

## TRP 0.104.1 (2026-10-23)

- The SKILLS tab now looks like your capture of Tarkov's: each skill's bar has its dark band, the small arrow at its end (skills level faster at a raid's start), and the SKILLS and MASTERING tabs have their icons. A skill's tooltip says "Leveling speed increased: 130%", as Tarkov's does in a raid.
- Clicking an item no longer makes the inventory blink.
- A dragged item now shows as just its picture, centred on the mouse, as in Tarkov. R turns it, square items too. The cells it would take light up green where it fits, yellow where it would merge or split, red where it can't go.

## TRP 0.104.0 (2026-10-23)

- The inventory's SKILLS tab is now Tarkov's own skills screen (with TARKOV UI on): every skill with its level, bar and buff icons, in a list or a grid, sorted and filtered as in Tarkov. Hover a skill's icon for its tooltip with each buff at your level; hover a buff's icon for its line.
- Skills that change nothing in this app yet are dimmed, as Tarkov dims a locked skill, and their tooltip says why. With the SKILLS switch off, every skill is dimmed.
- With TARKOV UI off, the tab is a plain list of every skill with its level and buffs.
- The MASTERING tab and the experience bar at the top are shown dimmed: neither is kept here.

## TRP 0.103.0 (2026-10-23)

- New SKILLS page in the options: set each of Tarkov's skills from 0 to 51 (51 is elite). Levels take effect at once, except a weapon's own skill and Aim Drills, which apply the next time you draw the weapon, as in Tarkov. Your levels are saved.
- The skills now change what they change in Tarkov: stamina and breath (Endurance); top speed, jump, carry limits and aiming fatigue (Strength); bleeding, fractures, pain, hunger, thirst, food and painkillers (the health skills); how loud your steps and gear are (Covert Movement); a weapon's reload, draw, handling and recoil (its class) and aiming speed (Aim Drills); magazine loading and checking (Mag Drills). At elite Strength your weapons weigh nothing.
- No skill points are earned. A game hosting this runtime can raise a level during play, and the SKILLS page shows and saves it.
- A new SKILLS switch under FEATURES (SIMPLE) turns all of this off. It's on by default, and every skill starts at 0, which changes nothing.


## TRP 0.102.1 (2026-10-23)

- No change in play. The skills are now written down in full: what each one changes at each level and at elite, and how each one is earned, checked against the game's own figures. Earning skills will come later as an option, off by default.

## TRP 0.102.0 (2026-10-23)

- The app now starts with textures and shadows at HIGH, instead of your install's own settings. Mip streaming stays on and Vulkan stays the renderer. A choice you saved in the options is kept, and AS THE INSTALL HAS THEM still sets them to your install's.

## TRP 0.101.1 (2026-10-23)

- After a death, the keypad's Enter now starts the run again too, as the main Enter always did. Before, it did nothing. 

## TRP 0.101.0 (2026-10-23)

- The options' FEATURES page is now three lists: SIMPLE (what you see and feel), EXPERIMENTAL (what differs from Tarkov on purpose) and ADVANCED (each system on its own).
- Three new SIMPLE switches turn a whole group off at once: STAMINA (drain and exhaustion), SOUNDS (movement, hands and shots) and VISUAL EFFECTS (muzzle flash, particles, spent cases, magazine rounds). Under ADVANCED each part still has its own switch.
- Renamed: DUMMY is TARGET DUMMIES, ADVANCED ATTACHMENT SYSTEM is ATTACHMENTS, HALL LAMPS is LEVEL LAMPS, and the old STAMINA is STAMINA DRAIN. If you had STAMINA off, exhaustion is now off too.
- LITE INVENTORY is listed under EXPERIMENTAL; it is not built yet.
- The list of what is not built is gone from the page.

## TRP 0.100.13 (2026-10-22)

- Nothing in the app changes. The maintainer's word on skills: what they are and what they change, never how they are earned; the documents say so.

## TRP 0.100.12 (2026-10-22)

- Nothing in the app changes. A new document, `TRP\docs\spec\skills.md`: what the install's own figures say about skills (read from SPT's published source, its 4.1.6 release), which skill effects the app already applies and from where, and what is still to read from the game's code; the wiki's figures for Covert Movement, Strength and Endurance noted as the wiki's until the code confirms them.

## TRP 0.100.11 (2026-10-22)

- Nothing in the app changes. Noted from the maintainer for the skills to come: the profile's skill levels change the game's sounds (Covert Movement quiets their footsteps by 49%) and other things; the figures will be read from the install when skills are built, and applied before any sound level is compared with a recording.

## TRP 0.100.10 (2026-10-22)

- Nothing in the app changes yet. The maintainer answered the fourteen open questions; what follows from them is on the list: the app will start at HIGH, a bot on the range comes before other third-person models, skill levels will be set in the options, the shader compiler's two DLLs will ship with a release, and four reports to look at (the respawn after death, the crates' opening sound, the HUD, a weight).

## TRP 0.100.9 (2026-10-22)

- The licence (Apache-2.0) covers the documents as well as the code: copy either with the notice.
- A stray model file at the repository's top (`Untitled.glb`, the maintainer's own range) is gone; the Forge's address in both READMEs is `sp-mod.com/installer`.

## TRP 0.100.8 (2026-10-22)

- For a coding agent working in the public copy: a short brief at the top (`AGENTS.md`) with the rules any change must keep and where things are. In `TRP/README.md`, "Reading this repository": the words the documents use, who "the user" is, and what can and cannot be checked against the game from this copy.

## TRP 0.100.7 (2026-10-22)

- For the AI sessions: Claude Code's auto-memory is off for this project (`.claude\settings.json`, private repository); the project's own documents are its memory.

## TRP 0.100.6 (2026-10-22)

- For the AI sessions: the last rule that lived only in the agent's notes on the user's PC is in `decisions.md` (private repository); those notes can be deleted.

## TRP 0.100.5 (2026-10-22)

- For the AI sessions: a one-line `CLAUDE.md` imports `AGENTS.md`, so that every Claude Code version loads the project's rules (older ones read only `CLAUDE.md`).

## TRP 0.100.4 (2026-10-22)

- For the AI sessions: every estimate and unconfirmed rule in the code is now listed in `TRP\docs\status.md`, "Placeholders, marked in the code", with its file, and the list names nothing the code does not mark.

## TRP 0.100.3 (2026-10-22)

- A capture can be cropped: `--screenshot-crop x,y,w,h` with `--screenshot` writes that region of the frame alone.
- The version now lives under `[workspace.metadata.trp]` in `TRP\Cargo.toml` (the crates' own version stays 0.0.0), so that raising it no longer rebuilds everything; the app prints it as before.
- The install-bound tests are one binary a crate (`--test sim`, `--test physics`), and read the item database once instead of forty-two times.
- For the AI sessions: the working notes' lists made one each (what is built, what is missing, what was compared), hooks against the shell forms that fail here, section banners in the largest source files, and a ledger check for every estimate in the code.

## TRP 0.100.2 (2026-10-22)

- Nothing in the app changes. The sessions' notes now say that the options' three lists (SIMPLE, EXPERIMENTAL, ADVANCED) are built and await their write-up as 0.101.0; the documents' reading tool lists a spec's rules and guards against printing a 40 KB section.

## TRP 0.100.1 (2026-10-21)

- Nothing in the app changes. The working notes for the AI sessions were cut to a third (`AGENTS.md`) and their repeated steps written as skills under `.claude\skills\` (private repository only); the manual's list of every version moved to `history.md`.

## TRP 0.100.0 (2026-10-21)

- The range is open to the sky again: the hall's walls, windows and roof are gone.
- Tarkov's lighting options (Customs light, camera colour filter, sky scattering, clouds, ambient highlight, Tarkov shading, hall lamps) are now off unless you turn them on.
- An armoured rig no longer shows its plates as special slots in your gear.
- The inspect window now shows every slot of the item and its parts, its buttons (reload, unload, equip, discard and the rest), a weapon's mastering, and a container's size.
- Rest the mouse on a row's little arrow to see more: a round's penetration against each armour class, an armour's areas and plates.
- Stretch the inspect window by its edges and corners.
- The inspect window's loading spinner now turns.

## TRP 0.99.0 (2026-10-21)

- The inspect window now lists a thing's stats as the game does: a weapon's durability, ergonomics, accuracy, recoil and the rest; a round's damage, penetration and speed; armour's points, penalties, zones and class.
- The inspect window shows the thing's 3D model, as the workbench does: drag across it to turn it, and use the mouse wheel to zoom.

## TRP 0.98.0 (2026-10-21)

- At the workbench the gun is put away while you edit, and drawn again when you close the screen, with a rack if you built something (nothing in it changes).
- Zoom the gun in and out with the mouse wheel on the workbench.
- Long attachments now sit in the same square boxes as in their lists, so the arrow that opens a slot's list is never covered.
- A gun can't be built while a required part is missing.
- The helmets' face shields, mandibles, ear covers and side armour are now in the HELMETS storage.

## TRP 0.97.0 (2026-10-20)

- The edit build screen now looks like Tarkov's own: its buttons, toggles, slot boxes with their part icons, the arrow that opens a slot's list, the list itself and the loading spinner.
- The range shows faintly through the screen's background.
- Clicking around the screen no longer flings the gun; only dragging turns it.
- Changing a part no longer makes the screen flash.
- Parts show a spinner while their pictures load (in the inventory too).
- The screen costs less to draw.

## TRP 0.96.1 (2026-10-20)

- The workbench is now just its main metal table, standing where the clothing crate was, with nothing else around it.
- Dragging the gun around in the edit build screen no longer fires it.

## TRP 0.96.0 (2026-10-20)

- The clothing crate is gone: in its place stands Tarkov's own hideout workbench (level 3). Its things moved to the matching cabinets and weapon cases, and the yard door's key lies on the bench.
- Stand at the workbench and press F for EDIT BUILD: Tarkov's screen for changing every part of the gun in your hands. Spin the gun by dragging, pick any part that fits from a slot's list, and press ASSEMBLE to build it at once. No trading needed.
- Guns now show and handle with the parts they actually have, in your hands, in the inventory and on the ground (ADVANCED ATTACHMENT SYSTEM). Both new systems can be switched off in the options.

## TRP 0.95.0 (2026-10-20)

- The practice dummy now holds an AKM in Tarkov's own idle stance instead of standing straight, and a second dummy next to it walks left and right on its own. When one dies it drops its rifle as it falls. F9 brings both back.

## TRP 0.94.0 (2026-10-20)

- A practice dummy now stands down the shooting lanes. It wears a copy of your gear: put on armour and a helmet, press F9, and it dresses the same (F9 also heals it and stands it back up). It takes hits and dies like a Tarkov character, falling as a ragdoll, with blood on hits. Start next to it with `--start dummy`; it can be turned off in the options (DUMMY).

## TRP 0.93.0 (2026-10-20)

- Bullets now know how to hit a person: body parts, armour plates, helmets and soft armour work as in Tarkov, down to the damage that gets through and where it lands. Nobody stands on the range to shoot at yet; the practice dummy comes next.

## TRP 0.92.0 (2026-10-20)

- Glass now throws its sparkling shards when hit, and a few drifting fragments are added to concrete hits. Every surface on the range now shows its full bullet-hit effect.

## TRP 0.91.0 (2026-10-20)

- Bullet hits now throw their small chips and specks: white specks off concrete, wood chips off wood, dark specks off plastic and soil, lit by the sun like the rest of the scene. Glass shards are still to come.

## TRP 0.90.0 (2026-10-20)

- More of each bullet hit's effect now shows: the spray of fine debris, the dust splash on concrete, wood, plastic and soil, and the splashes and mist on water. The small flying chips and specks and the glass shards are still to come.

## TRP 0.89.0 (2026-10-20)

- Bullet hits now show and sound like Tarkov's. Each surface leaves its own bullet hole (concrete, wood, metal, soil, plastic) and throws up its own dust, chips and sparks, metal and concrete with a quick flash of light, and each has its own impact sound; a ricochet whines. Holes in a door swing with it when it opens. A few of the hit effects (some dust, splinters and glass shards) are still missing and come next.

## TRP 0.88.0 (2026-10-20)

- Shots now fire real bullets. Each one flies as in Tarkov, slowing in the air and dropping, and hits the range's walls, crates, doors and ground. Depending on the material it may go through, change direction, break into pieces, glance off or stop, by Tarkov's own rules and odds. For now nothing shows where a bullet hits (bullet holes, dust and impact sounds come next). It can be turned off in the options (FEATURES, BALLISTICS).

## TRP 0.87.0 (2026-10-20)

- Firing the RShG-2 now has its launch blasts: a burst of smoke and sparks out of the front of the tube, and a fiery back blast with smoke, ash and sparks out of the back that lights up everything behind you in orange.

## TRP 0.86.0 (2026-10-20)

- A flashbang now has its own effect: a burst of sparks and puffs of smoke, and a bright flash of light that lights up everything around it and casts shadows, fading over a second.

## TRP 0.85.0 (2026-10-20)

- A frag grenade's explosion is now complete: chips of concrete fly out, bounce off the floor and walls and kick up dust where they land; the flash lights up the walls and floor around it; a shock wave and a heat shimmer ripple the air (subtly, as in Tarkov); and it leaves a scorch mark on the ground. All of it turns off with PARTICLES in the options.

## TRP 0.84.0 (2026-10-20)

- Surfaces are now lit the way Tarkov's renderer lights them: the sun, the sky's light, the hall's lamps and a grenade's flash all follow the game's own lighting. Sunlit sides come out brighter, and metal in the shade darker. Inside the hall the lamps are as bright as Tarkov's lamp would be at that height, so the floor is lighter. It can be turned off in the options (FEATURES, TARKOV SHADING) to go back to the old lighting.

## TRP 0.83.0 (2026-10-19)

- Your arms and weapon get Tarkov's extra light outdoors: the game lights them a second time with the light from around and below, so they don't go dark in the shade. Indoors it adds nothing, as in Tarkov. It can be turned off in the options (FEATURES, AMBIENT HIGHLIGHT).
- Inside the hall, the light the lamps bounce off the floor and walls now lights everything, you included, standing in for the baked light Tarkov's buildings have: the ceiling and upper walls are no longer black.

## TRP 0.82.1 (2026-10-19)

- The hall's ceiling lamps now shine white instead of orange, so the inside looks brighter.

## TRP 0.82.0 (2026-10-19)

- Clouds over Customs, as Tarkov draws them: a layer of clouds lit by the sun and the sky, drifting with the wind, whose shadows pass over the ground (shadows need the Vulkan renderer). CLOUDS QUALITY is now on the GRAPHICS page (restart required), starting as your Tarkov has it. The clouds can be turned off in the options (FEATURES, CUSTOMS CLOUDS).
- The moon is in the sky: faint by day, low on the horizon opposite the sun.

## TRP 0.81.0 (2026-10-19)

- A frame rate limit, as in Tarkov: GRAPHICS, GAME FPS LIMIT (30 to 144) and DISABLE GAME FPS LIMIT, starting as your Tarkov has them. As in Tarkov's raids, the limit holds with VSync off; with VSync on the screen sets the pace.
- MSAA can be chosen: GRAPHICS, MSAA (off, 2x or 4x).

## TRP 0.80.0 (2026-10-19)

- Faster: a frame now costs about 40% less than in 0.78.0, ambient occlusion included. Clothing and gear that are not being worn no longer slow the game down, and frames are now drawn on a separate thread, as Tarkov draws them. That can be turned off in the options (GRAPHICS, RENDER THREAD; restart required).

## TRP 0.79.0 (2026-10-19)

- Ambient occlusion, as Tarkov draws it: corners, the foot of walls and the ground around things are darkened by Tarkov's own SSAO, at the setting your Tarkov uses. The setting is now on the GRAPHICS page (SSAO; restart required). It can be turned off in the options (FEATURES, AMBIENT OCCLUSION).

## TRP 0.78.0 (2026-10-19)

- The range is now the one made in Blender: high windows at the far end of both long walls, cover in the lanes, the barbed wire past the low wall, and the surfaces sampler as a course of strips outside the hall's wide opening. The LEVEL FILE option is gone; to try an edited range, start with `--level <file>`.

## TRP 0.77.1 (2026-10-19)

- Your own range from Blender is in: in a level file, each square of the surfaces sampler now goes where you put it and as large as you made it, a start point follows its arrow when you move or turn the arrow, and a cube left without a material is concrete grey instead of white.

## TRP 0.77.0 (2026-10-19)

- The range can be edited in Blender: write it out with `--export-level local\level\range.gltf`, import it in Blender (File > Import > glTF 2.0), move, resize, turn, copy or delete its walls, floors and blocks, move and turn the crates, tables, cabinets, doors and the car, the start points, the lamps and the indoor area, then export it as `level.glb` into the `local\level` folder. The app reads it at the start. It can be turned off in the options (FEATURES, LEVEL FILE).

## TRP 0.76.1 (2026-10-19)

- Fixed: outdoors near the hall, looking away from it, the shaded sides of things went black and the whole picture turned orange-brown.
- The range is now indoors: a long hall in the style of Tarkov Arena's shooting range, with high windows the morning sun shines through, windows and a wide opening to the outdoors, and the doors to the yard. Inside, Tarkov's camera adjusts as it does indoors, and the sky's light is shut out as Tarkov's buildings shut it out. Ceiling lamps light the hall; they can be turned off in the options (FEATURES, HALL LAMPS). The stairs and the platforms for falling are outside, by the hall's wall.
- The range is brighter: a neutral grey ground and light concrete walls, stairs and platforms, as light as Tarkov's camera allows before it turns the exposure down. The picture is now the same brightness every time you start.
- Storage along the hall's wall, one of every such item in the game in each: cabinets for rigs, backpacks, helmets, armour and armoured rigs, masks and hats, and headsets, a box of armour plates, and a dresser of eyewear (helmets and armour with their plates; no quest items).
- The barbed wire has moved behind the start, to the left of the doors.
- Distant things now fade into the air toward the sky's colour, as Tarkov's camera draws it on Customs. It can be turned off in the options (FEATURES, SKY SCATTERING).
- Tarkov's camera's own colour filter, which every map's camera has on as the game ships it, now lightens the middle tones and the sky a little. This is not the optional colour filter in Tarkov's Post-FX settings. It can be turned off in the options (FEATURES, CAMERA COLOR FILTER).
- The range is now lit as Tarkov's Customs on a clear morning: the sun, the sky and the light in the shade are worked out by the game's own time of day, and the image goes through Tarkov's own camera, which adjusts to how bright the view is, darkens the corners and tones the colours. Smoke, dust and fire are lit the same way. It can be turned off in the options (FEATURES, CUSTOMS LIGHT).
- The OVERALL tab now works, as in Tarkov: your character's model (drag across it to turn it), your level and statistics (dimmed for now: none are kept yet).
- Clothing: two dropdowns under the model change your upper and lower clothing, from every suit in the game, USEC, BEAR and Scav alike. A suit loads the first time you pick it and changes your body, your model and your hands in first person; your choice is kept for next time. It can be turned off in the options (FEATURES, CLOTHING).
- Gun smoke puffs now overlap in the same order as in Tarkov.
- Grenades now explode as in Tarkov: a fireball, sparks and dirt thrown up, a cloud of smoke that spreads and thins, and dust along the ground (with PARTICLES).
- Sparks: now and then a shot throws glowing sparks from the muzzle that streak, swirl and fade, as in Tarkov (with PARTICLES).
- Heat haze: the air at the muzzle shimmers and blurs what is behind it for a moment after a shot, as in Tarkov (with PARTICLES).
- Particle effects: each shot now throws puffs of smoke from the muzzle and the ejection port that drift and fade, and a burst leaves a haze hanging in the air, as in Tarkov. It can be turned off in the options (FEATURES, PARTICLES).
- Dropped magazines now reuse the textures already loaded for the weapon in hand, instead of loading them again.
- The StG 44 now has a muzzle flash and gun smoke, shows rounds in its magazine, and its magazine looks like its own when dropped and in the inventory (it showed the SCAR's).
- Fixed: firing some weapons crashed the game (17 of them, including suppressed ones like the VAL, VSS and PB). Every weapon has now been test-fired.
- Gun smoke: after a burst, a thin trail of smoke rises and curls from the muzzle as you move the weapon, as in Tarkov (with MUZZLE FLASH).
- Muzzle flash: firing now shows the weapon's own flash at the muzzle, as in Tarkov. It can be turned off in the options (FEATURES, MUZZLE FLASH).
- Blood on the screen: hits and bleeding leave drops of blood on your view that fade, and heavy bleeding makes the edges pulse red, as in Tarkov. All the screen effects now have Tarkov's own colours (the near-death border is a deeper red).
- Getting hurt now shows on screen as in Tarkov: a hit (barbed wire, a grenade, F7) jolts and blurs your view for a moment, pain makes it blur and pulse, exhaustion darkens its edges, and at low health a red border creeps in.
- After a flash grenade, a burnt-in spot stays where it went off (with a trail if you were turning), fading slowly, as in Tarkov.
- Flash grenades now blind and stun you when you look toward them: a white flash, then a doubled view while stunned, and a reload while stunned is always the quick one. Face shields and visors protect your eyes. Low health now also makes your view waver and blur, as in Tarkov.
- A concussion now blurs and doubles your view and makes it waver, fading as the concussion wears off, as in Tarkov.
- A concussion now makes your ears ring, as in Tarkov: the world goes quiet for a few seconds and comes back while the ringing fades. Headphones protect you from it.
- Grenades now sound like Tarkov's when they go off, heard a moment later the farther away you are, and a blast nearby shakes your view.
- Magazines now show their rounds, as in Tarkov: at the top of a loaded magazine lying on the ground, in the magazine in your weapon (they go as you fire), and in a loaded magazine's inventory picture. It can be turned off in the options (FEATURES, MAGAZINE ROUNDS).
- Spent cases: firing throws the empty case out of the weapon's ejection port. It bounces on the ground with Tarkov's own sounds (they differ by the bullet and what it lands on) and disappears after a second, as in Tarkov. It can be turned off in the options (FEATURES, SPENT CASES).
- Loading, unloading or checking a magazine now shows Tarkov's spinner on it (point at it and it turns red with a cross: click to stop).
- Grenades now hurt you: stand too close to a blast and it damages every part of your body it can reach (walls and crates protect you). A stun grenade near you gives you a concussion.
- The wish list: right-click a thing, point at ADD TO WISHLIST and pick a group (or REMOVE FROM WISHLIST). Things on your wish list are marked in the inventory.
- Magazines and cases can be tagged: right-click, TAG, type a name and pick a colour, SAVE. The tag shows on the item (with its name where there is room) and in its window. RESET TAG takes it off.
- Right-click a magazine you found and choose CHECK MAGAZINE: after 3 seconds you know how many rounds are in it.
- Weapons with a folding stock can be folded and unfolded from the right-click menu (FOLD, UNFOLD), in your hands or in a bag. Folded, they take less room.
- Some weapons' sizes in the inventory now match Tarkov (a silencer that adds length, like on the Model 700).
- The right-click menu's icons are no longer stretched.
- Fixed: the inventory screen was redrawing itself on every frame while open.
- A folded weapon has its own inventory picture, with the stock drawn folded.
- A shotgun's tube now shows the shells in it (it showed 0/5 while loaded).
- Drop a round on a weapon with no magazine to put it in the chamber; for the weapon in your hands, your character loads it by hand.
- Right-click a weapon in your bag and choose DISASSEMBLE to take its parts off (in a raid the parts it can't do without stay on).
- Firing now takes rounds out of the magazine you see in the inventory, and after a shot you no longer know exactly how many are left (it shows ?), as in Tarkov. Check it to know again.
- Face shields and night vision can be flipped down and up: press N, or right-click the helmet and choose FLIP DOWN or FLIP UP. You hear it, and with N your character reaches up to do it.
- A helmet's parts are drawn on your character: a face shield fitted to it shows, and swings up and down on its hinge as in Tarkov.
- A weapon's right-click menu now works for its magazine: UNLOAD, LOAD and RELOAD, and INSTALL or UNINSTALL on a magazine. For the weapon in your hands, your character takes the magazine out or puts it in with their hands, as in Tarkov.
- Magazines load and unload one round at a time, at Tarkov's own speed and with its sounds: drop rounds on a magazine to load it, or right-click a magazine and choose UNLOAD AMMO. Click the magazine to stop.
- Grenades: press G to take one in hand, hold the left mouse button to pull the pin and let go to throw (the right button throws underhand). It flies and bounces, and is gone after its fuse. There is no explosion yet.
- The knife: press U to take it in hand, then left click to slash and right click to stab. Double-tap U for a quick stroke.
- A new GRENADES table on the range has every grenade and melee weapon.
- Grenades and melee weapons now have their inventory pictures.
- When you die, the screen fades to black over a second, as in the game, before the death screen.
- Grenades and the knife can be turned off with Options > FEATURES > GRENADES AND KNIFE.

## TRP 0.42.4 (2026-10-17)

- Tarkov's interface is now on by default. To use the app's own look instead, turn off Options > FEATURES > TARKOV UI.

## TRP 0.42.3 (2026-10-17)

Everything new since 0.9.1.

### Install and play

- Double-click `Install.bat` once. It installs Rust and the C++ build tools if they are missing (it asks first), builds TRP, and asks where your SPT, GAMMA and Anomaly folders are.
- Double-click `Play.bat` to play. It rebuilds by itself after an update.
- About 10 GB of disk space in all.

### Health

- Seven body parts, with bleeding, fractures, pain, hunger, thirst, falls and barbed wire.
- Medicine, food and drink: use them from the quick slots, by double-clicking, or by dragging them onto a body part.
- Death as a ragdoll on Tarkov's own physics engine. Press `Enter` to start again.
- For testing: `F7` hurts you, `F8` kills you.

### Tarkov's interface (Options > FEATURES > TARKOV UI)

- The inventory, health screen, loot panel, quick-use bar, inspect window and right-click menu, drawn as Tarkov's own from your install.
- The in-raid HUD: stance, health and the quick bar.

### Sound

- Interface and item sounds.
- Footsteps on the game's 20 surfaces, plus jumping, landing and crawling.
- Weapon handling, gunshots and aiming.
- Mixed with the game's own mixer settings.

### Graphics and performance

- Vulkan by default, DirectX 12 in the options.
- Tarkov's texture quality, texture streaming and shadow settings.
- Faster frames, a smaller build, and fixes for several crashes.

### Other

- Dropped items fall and land with real physics.
- A switch in the options for every system, and new GAME and SOUND pages.
- `F1` shows the debug panel and the keys.
- More weapons checked against recordings of the real game.

### Not done yet

- Grenades, the knife, malfunctions, ballistics, optics, slopes, a second character, skills, the world's sounds, and the S.T.A.L.K.E.R. host.
- The sound levels have not been checked against the real game yet.
- `Install.bat` installing Rust and the C++ tools has not been tried on a PC without them. If it fails, it opens the download page.

### Requirements

- Windows 10 or 11 and a desktop graphics card.
- Your own Escape from Tarkov with SPT 4.1 installed over it (client 0.16.9.5, SPT 4.1.6).
- Optional: S.T.A.L.K.E.R. Anomaly or GAMMA, for its suits.

## TRP 0.9.1 (2026-10-14)

Every weapon on the range, the character model on the gear page, the inventory's icons, and a performance pass.

## TRP 0.1.0 (2026-10-14)

The first public version.
