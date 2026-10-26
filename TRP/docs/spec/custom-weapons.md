# A weapon of one's own: the Sturmgewehr 44

How a weapon that is not in the install is made to work here: its model on the rig of one of the install's weapons, worked by that weapon's animator, with the animation edited where the two do not fit. Written on 2026-10-13, when the user gave a model of a Sturmgewehr 44 (`local\ExperimentalWeapon\STG44`) and asked that the best of Tarkov's animations be found for it, edited as needed, with the weapon under every rule Tarkov's own are under. **Nothing here is compared with the game: the game has no such weapon.** What is measured is said to be, and against what.

Nothing of the model is in the repository. Its file and its pictures stay in `local\`; the repository has the code and one file of figures, `custom\stg44.json`. Where the model is not there, the weapon is left out and everything else runs as before.

## The rules

A weapon of one's own is a weapon like the install's in everything the controller does, because it is made one of them:

- **It is an item of the database.** Its template is its donor's with what its file changes, it has a magazine of its own, and a stock preset of the two (`trp_data::custom::register`). So its weight, its cells, its rate of fire, its rounds, its recoil, its slot and what the inventory makes of it all come from a template by the code that reads every other weapon's.
- **It is assembled as the game assembles a weapon** (`trp_data::weapon::assemble`): the base prefab, here the donor's own and bare, and on it the bones a weapon's parts bring (the muzzle, the sights' two points and the eye's bone). What is drawn is the model's parts, each a node on one of the donor's bones, in place of the donor's own.
- **Its logic is the game's operations on the donor's animator**, untouched: the states, their times, their marks and their conditions are the donor's (`firing.md`). Which of them run goes by the template, as for any weapon: with no catch for its bolt the donor's animator takes its other way through a reload.
- **The hands are solved onto the rig's markers** as on any weapon (`hands-on-weapon.md`), the eye goes to the sights' line (`weapon-handling.md`, "The sights"), the line that feels for obstacles runs to the muzzle, and the body's animator is told its width and its kind.

What is its own: the model, where the model lies on the rig, and **the edits to the pose** (below), which are the only thing here that no weapon of the install has.

## Choosing the donor

"Best" is taken to mean: the donor whose animations need the least editing to be right for this rifle. That is measured, not judged by the weapon's name.

**What the rifle is** (the model, in its own centimetres; `probe fbx <file> view <png>` draws it over a grid, `probe fbx <file> slice ...` gives the corners in a slab). 92.9 cm long. The bore 14.1 cm over the model's origin, the gas tube 2.3 cm over that. A pistol grip, the trigger ahead of it, and the magazine well ahead of the trigger guard, the magazine straight up into it. **The cocking handle is on the left**, on the gas piston's rod above the magazine, in a slot 17.7 cm long, and travels with the bolt at every shot. A wooden stock that does not fold. A rear sight with a notch and a hooded post at the muzzle, 44.4 cm apart. No part of it holds the bolt open on an empty magazine (**PROVISIONAL**: from what is generally written of the rifle; the page read for its figures does not say).

**What a donor has to have**, in the order it decides:

1. A magazine that comes out, the two ways of firing (single shots and automatic), and an animator that works the action after a reload from empty when nothing holds the bolt open.
2. **The hand that works the action, and on which side.** This cannot be edited in: a hand that reaches over to the right of the weapon is turned for it.
3. Then how far its things are from this rifle's, with the right hand on the grip: the handle, the magazine well, the bore, the muzzle and the butt.

**Measured** (`cargo run -p trp-sim --bin donor -- --no-catch`; kept as `local\donor_all_nocatch_s42.txt`): every rifle, carbine, submachine gun and machine gun of the install that has a stock preset (91 of them; seven have none), each run through a draw with nothing chambered and a reload from empty by the simulation's own logic on its own animator, as a weapon with no catch would run it. For each: which bone travels furthest back, how far, and which hand has left its hold at that moment. **In the draw 57 are worked by the right hand and 34 by the left.**

- **Of the 47 weapons the install itself gives no catch, 41 are worked by the right hand**: the whole Kalashnikov family, the Vepr, the VSS and the Val, the SR-3M, the 9A-91, the RPD, the PKM, the M60, the PPSh. The others are the P90, the three Uzi and the SR-2M, none of them a rifle with its magazine ahead of the grip (and the RPK-16, in whose draw and reload nothing travelled at all: not looked into).
- **The AR-15 family** (and the older MCX) is worked by the right hand in a draw and by the left after a reload, from behind, by the handle at the back of the receiver's top. The MCX Spear is worked the same way by the left; its side handle is a bone of its rig that these states do not move.
- **Of the rest of the 34 worked by the left hand**, the MDR, the RFB, the Aug, the P90 and the Vector have the magazine behind the grip or the action under the hand; the MP5, the MP7, the MP9, the MPX, the PP-91, the Uzi and the SR-2M are worked from on top or far ahead. **On the left side of a rifle with its magazine ahead of the trigger and both ways of firing** there are the FN SCAR-L and SCAR-H, the DSA SA-58, and near them the HK UMP (its handle far ahead) and the HK G36 (its handle on top). All of these have a catch in the game; all work the action after a reload from empty when run without one. The MP5 and the Aug A1 do not: without a catch they still lock the bolt open first and slap it shut. The Mk-18 and the RSASS fire single shots only.

**Laid to the hand.** A donor's trigger mesh is no reference (it has the sear's arms on it), so the model is laid to the donor's right hand as the donor's resting clip has it: the forefinger's last joint 0.75 cm ahead of the trigger's face at the trigger's middle. Then, with the model so laid (`donor --overlay <weapon> <png> --model <fbx> --place ...` draws both, with the hands' bones):

| With the right hand on the grip | FN SCAR-L | DSA SA-58 | AKM | HK UMP |
|---|---|---|---|---|
| The hand that works the action | left | left | right | left |
| This rifle's handle from the donor's | 6.9 cm back, level, same side | 5.8 cm back, 4.3 cm higher | the other side | 14.9 cm back |
| This rifle's magazine well from the donor's | 5.3 cm ahead | 2.5 cm ahead | 3 cm ahead | |
| This rifle's bore over the donor's | 0.7 cm | 1.8 cm | 2.7 cm | 1.1 cm |
| This rifle's muzzle from the donor's | 0.5 cm short | (a short barrel) | 2.7 cm ahead | |
| This rifle's butt from the donor's | 0.7 cm back | 0.7 cm back | 2.8 cm back | |

**The donor is the FN SCAR-L** (`6184055050224f204c1da540`). With the right hand on the grip the two rifles are the same to within a centimetre in their bore, their muzzle and their butt, so everything that goes by where the weapon is (the eye, the shoulder, the sprint's carry, the line that feels for obstacles) needs nothing. Its handle is on the left at this rifle's own height and goes back and forth with its bolt at every shot, as this rifle's does. Its magazine goes straight in. Its selector is where this rifle's is (1.7 cm from it). What does not fit is two shifts along the barrel, the handle's and the magazine's, and where the left hand rests; those are the edits.

The SA-58 is nearer in its magazine and no nearer in its handle, which is 4 cm lower, does not move at a shot, and its magazine rocks in. The AKM, the nearest in looks and in its round, is worked by the wrong hand on the wrong side.

## The catalogue of donors (2026-10-13)

The measuring above, kept for every weapon so that the next weapon of one's own, or a model taken from another game, is a look-up and not a day's work. **It is a file of figures made from the install on the user's machine** (`local\donors\catalogue.tsv`, ignored: no game content is in the repository), and it is made again whenever the install changes.

- **`cargo run -p trp-sim --bin donor -- --catalogue [file]`** (about a minute) runs every weapon of the database that has a stock preset and that this simulation can work through a draw with nothing chambered and a reload from empty, on its own animator, and writes **one line for each different set of clips**: 95 donors of 146 weapons, each with the weapons that share its clips. A line has: the weapon, its class, its rounds, its catch, its rate and its ways of firing, whether its magazine stays in it, whether its bolt is worked by hand, whether it is a pistol; where its bore, its muzzle, its butt and its magazine are from the right hand's grip; where the two hands rest; and for the draw and for the reload the hand that works the action, the part it moves, how far that travels and where the palm is then; and the reload's states in their order.
- **A weapon of one's own says what it wants** in its file's `wanted` block (`custom\stg44.json`): its kind (`rifle`, `pistol`, `bolt`, `shotgun`, `tube`), the hand that works its action and the side that is on, whether it fires automatically, and where its bore, muzzle, butt, magazine and handle are with the right hand on the grip, in centimetres.
- **`donor --rank <the weapon's file> [catalogue]`** lists the donors by the rules above in their order: the kind and the ways of firing decide who is listed at all; then the hand and the side, which no edit of a pose can change; then how far the donor's things are from this weapon's.

**Checked against the choice made by hand:** ranked from the StG 44's figures alone, the catalogue's first is the FN SCAR-L (8.9 cm in all), then the HK G36 (9.1), the SCAR-H (10.1) and the SA-58 (15.3): the same donor, and the same runners-up, as the day of measuring gave.

**Not in it:** a weapon with no stock preset (fourteen) and the TOZ-106, whose animator the run cannot fire; how the fingers close. **A revolver, a launcher and a break-action have a line, but no bolt to measure**: their "part that travels furthest" is whatever moved most, and says little; `--rank` has no kind for them yet. A rank is where to look first: the model still has to be laid to the donor's hand and the edits measured (below).

## The file

`custom\stg44.json` (`trp_data::custom::Definition`). Every length is the model's own (its units and axes: x to the right, y up, z back along the barrel) unless said otherwise.

- **`donor`**: the SCAR-L's item template.
- **`model.origin`**: where the model's origin lies on the donor's weapon bone, as centimetres ahead, up and to the right: **42.05, -13.0, 0**. From the forefinger (above): its last joint rests at 31.8 ahead and 6.4 below the bone, the trigger's face is at z 11.0 and its middle at y 6.6.
- **`parts`**: each of the model's parts with the donor's bone it moves with. The receiver, the stock, the barrel with its handguard, the two parts of the rear sight and the safety's lever on the weapon's own bone; **the bolt and the piston with the cocking handle on the donor's bolt** (`weapon_bolt`), so that they travel as its bolt does, 10.6 cm, in a slot that has 17.7; the trigger on the donor's trigger; the two ends of the selector's button on the donor's selector; the magazine on `mod_magazine`. The sling is left out: no weapon of the install has one.
- **A part's `pivot`** is the point of the model the donor's bone is to be at, which is the point the part turns about: the trigger's own pin; the middle of the selector's button (the donor's selector turns about the axis across the weapon, and a round button turned about its own axis stays where it is); and for the magazine the front of its top, 0.55 cm ahead of it, as the donor's bone lies to the donor's magazine. The bone is moved there (the first edit, below).
- **`bones`**: `fireport` at the muzzle's middle (z -46.42, y 14.145); `mod_align_rear` at the top of the notch's rear face (z 0.40, y 19.94) and `mod_align_front` at the top of the post (z -44.03, y 19.98); `mod_aim_camera` on that line where the donor's own sight has the eye, 7.6 cm ahead of the weapon's bone, turned as the donor's `aim_camera`. The line they give is 0.05 degrees from level.
- **`hands`**: the left hand's three holds (the second edit).
- **`muzzle_from`**: the install's weapon whose muzzle effects it has (below, "The muzzle"): the SKS's (`574d967124597745970e7c94`).
- **`template`** and **`magazine`**: what the two templates have other than what they are made from (below).

## The animation, edited

The donor's clips are not rewritten. **The edits are rules on the pose the donor's animator gives out** (`trp_data::custom::PoseEdits`, applied in `trp_sim::weapon_animator`), each of which is the same as an edit to every clip at once, and holds through every state and every blend between two:

1. **A bone moved.** The same shift is added to where the animator has the bone, whatever the bone is doing: as if every clip's curve for it had been moved by that much. The part on it then turns about a point of its own, and travels the donor's path from this rifle's own place. So moved: the magazine (5.30 cm ahead, 0.48 up), the trigger (1.00 ahead, 0.61 up) and the selector (1.47 back, 0.95 up).

2. **The left hand follows what it holds.** The left palm's marker is shifted by a blend of the shifts of the things it is near. A thing ("hold") is at one of the donor's bones; while the donor's hand is on it the palm's marker is at a known place from that bone; and on this rifle the same thing lies a known shift from there. Each hold's shift is weighted by the palm's distance from where it would be on that hold, to the power of -3 (half a centimetre added, so that a palm exactly on a hold gives no endless weight), and the weights are made to add up to one. So on a hold the hand is shifted by that hold's own shift, exactly, wherever the thing has been carried to; between two it goes smoothly from one to the other; far from all of them it takes their mean, which nothing shows. The marker's fixed reference (`weapon_L_hand_marker`, by which the game knows the hand is "on the weapon") moves with the hold the hand rests in.

   | Hold | The donor's bone | The palm's marker from that bone on the donor (cm ahead, up, right) | This rifle's from the donor's |
   |---|---|---|---|
   | The handguard, at rest | the weapon's own | 45.9, -5.0, -6.0 (the marker at rest) | 5.6 ahead, 2.0 up, 0.9 to the right |
   | The magazine | `mod_magazine` | -9.9, -16.8, -8.2 taking it out; -9.8, -22.3, -2.8 pushing one home | the magazine's own move |
   | The cocking handle | `weapon_bolt/mod_charge` | -9.5, -3.3, -12.7 through the pull | 6.9 back, 1.1 to the right |

   The palms' places are read off the donor's own reload from empty (`donor <weapon> --no-catch --detail --from-bone`: the left palm, the magazine's bone and the bolt's travel every twentieth of a second; kept as `local\donor_mk16_detail_s42.txt`). The shifts are read off the drawings of the two laid together, to a millimetre or two: the donor's handguard's underside is 2.0 cm under the weapon's bone and this rifle's level with it; the palm keeps its distance from the front of the magazine well, which is 5.6 cm further ahead on this rifle; this rifle's handguard is 0.9 cm narrower a side; the donor's handle is 47.5 cm ahead, 3.4 up and 3.3 to the left, this rifle's knob 40.6, 3.46 and 2.2.

**What the edits do not touch**: the animator's states, times, marks and parameters; the right hand's marker, the weapon's own bone and the camera's, which are what the simulation takes from the rig; every turning. So the simulation's results for this weapon are those the donor's animator gives, and the edits are in what is drawn.

**Measured against the donor, frame for frame** (`a_weapon_of_ones_own_is_worked_by_its_donors_animator`: the StG 44 and the SCAR-L with no catch, the same rate of fire and the same round, stepped side by side through a reload from empty, a shot, a switch of fire mode, a look at the magazine, a look into the chamber and a second of automatic fire): the same things said and the same state, time through it and state being entered on every frame; the right hand's marker and the handle's travel the donor's to a hundredth of a millimetre; at rest the left palm 5.6, 2.0 and 0.9 cm from the donor's; **while the donor's hand has its magazine, this rifle's hand is within 0.1 mm of the same hold on its own**; at the handle's full pull (10.6 cm) the palm is 6.90 cm back and 1.10 cm to the right of the donor's. Held for a second on automatic it fires nine rounds.

## The two templates

**The weapon's** is the SCAR-L's, so that everything bound to the rig is the rig's (where it turns about, where its recoil pushes, its planes for aiming), with:

| Property | Value | From |
|---|---|---|
| `isBoltCatch` | false | the rifle (PROVISIONAL, above) |
| `Foldable`, `FoldedSlot` | false, none | its stock does not fold |
| `CompactHandling` | false | it always reaches the shoulder: the donor's own word for that is on its stock, a part this weapon has not |
| `bFirerate` | 540 | the rifle's rate, as published (500 to 600 a minute; 540 in the text of the page read) |
| What it weighs with its empty magazine | 4.6 kg | published; the template's own weight is that less the magazine's, 4.23 kg |
| `Width`, `Height` | 5, 1 | **ESTIMATE**: 94 cm; the install's AKM, 88 cm, is five cells as built; the magazine adds the second row |
| `ammoCaliber`, `defAmmo`, `Chambers` | the AKM's | **PROVISIONAL**: the install has no 7.92x33. Its 7.62x39 is the nearest it has (a round of 16 g for the rifle's 17, a bullet of 7.9 g at 715 m/s for 8.1 g at 685) |
| Every figure of its recoil, and `Ergonomics` | the AKM's as its stock build has them | **ESTIMATE**: the install's rifle of that round. The parts' shares the game adds up over a build (their `Recoil` percentages, their `Ergonomics`) are those of the AKM's stock preset, since this weapon has no parts to have any |
| `Slots` | the magazine's alone, taking its own magazine | it is one piece |

Loaded, the weapon weighs 5.10 kg here (4.6 and thirty-one rounds of 16 g); the rifle's published loaded weight is 5.13.

**The magazine's** is the SCAR-L's own (for the number that tells the animator which reload clip to play), holding what the AKM's steel thirty-round magazine holds and weighing what it weighs (0.37 kg: **ESTIMATE**, the install's magazine of the same make and size).

The published figures are from the English Wikipedia's page on the StG 44, read 2026-10-13.

## Built (2026-10-13)

- `crates/trp-data/src/fbx.rs`: a reader for binary FBX files (the records, the models' places, the meshes as triangles with their connected pieces), written from the published accounts of the format.
- `crates/trp-data/src/custom.rs`: the file, the two templates and the preset, the build on the donor's rig, the meshes (mirrored into the rig's axes and turned the right way out again, a tangent at every vertex), the pictures (read, halved to their limit, laid bottom row first like the install's, every smaller size made), and the pose's edits.
- `crates/trp-data/src/sketch.rs` with `probe fbx`, `probe overlay`, `probe custom` and `crates/trp-sim/src/bin/donor.rs`: the drawings and figures all of the above was measured with.
- The app builds every weapon of one's own after its four, draws its parts with its own pictures, and puts it in the crate like the rest (`Options::weapons`).
- **Its pictures are compressed as the install's are** (2026-10-14, night; the user: "Definitely compress any custom models to make them cheaper. I really really care about performance."): its colour and its surface as BC1 (the install's DXT1, half a byte a pixel where it had been four), its normals as BC5 (a byte a pixel where it had been two), every level (`crates/trp-data/src/compress.rs`, the project's own encoders: a block's main axis cut to its colours, fitted again by least squares). The StG 44's four materials at 2048 pixels: **42.7 MB where they had been 213.3 MB**. What comes back, against what was read (a root mean square in 255, `a_weapon_of_one_s_own_s_pictures_compress_closely`): its colour 2.3 to 3.8, its normals 0.4 to 1.1, its surface 3.4 to 5.6 (its three channels, occlusion, roughness and metal, share one line of colours a block: up to 9.3% of its pixels more than 8 off). **Kept compressed** in `local\picture_cache\v1`, by what the pictures come from (their paths, sizes and times, the size asked and the way the normals are turned): its rig is made ready in 0.28 s where reading the PNG files and compressing them takes 1.13 (0.81 before they were compressed). Delete the folder to have them made again.

**Run in the app** by scripted captures (`--weapon 66f0a5740000000000000044`): at rest in first person and from the side; with the sights up (the post stands in the notch); through a reload (the hand on the magazine, the new one coming); taken out of the crate and drawn, which works the action as for every weapon found in a container. Not played by hand by the one who wrote it.

## The muzzle, the magazine's rounds, the magazine as a thing (2026-10-18, 0.65.0)

The user (2026-10-18): "Also check custom weapons like the stg, they're missing the overhaul (Flash, smoke, casing, ammo in magazines) I believe." They were, all but the cases: the game takes a weapon's flash and smoke from its parts and a magazine's rounds' points from its magazine's model, and this weapon had neither of the install's. Each is now taken from the install as the rest of it is, and said here.

**The muzzle.** The game draws a weapon's flash and smoke from the objects of its parts that carry a `MuzzleEffect`, the deepest of them (`firing.md`, "The muzzle flash"), with its own `MuzzleManager`'s picture and timing. The model has no such part, and the donor's bare prefab none either. The file names a weapon of the install whose muzzle effects it takes (`muzzle_from`): those objects, with their scripts, are put below this weapon's `fireport` where they are from that weapon's fireport in the axes of its weapon bone, and read with that weapon's manager (`trp_data::custom::take_muzzle`). The receiver's `extractor_smoke` is not taken (it is at the ejection port, not the muzzle, and its fume is not drawn).

**Which weapon's** (this work's choice, `PROVISIONAL`): the StG 44's muzzle is bare (a cap over its thread), and it fires the install's 7.62x39. Of the install's weapons of that round (`probe muzzles 762x39`, `local\muzzles_762x39_s92.txt`), all but three have their flash on a muzzle device; the SKS's (both) and the RPD's are on the bare barrel, one jet with its smoke, half a centimetre (SKS) or 1.45 (RPD) ahead of the fireport. The SKS is a rifle: **the TOZ SKS's** (its barrel's `muzzleflash_000`: one jet, one smoke, 0.49 cm ahead).

**The magazine's object.** The game hangs a magazine's prefab on the slot's bone, and goes by that object: a reload shows and hides it with all it holds, its rounds' points (`patron_`) are found below it (`inventory.md`, "A magazine's rounds drawn"), and it is what is let go. Here the magazine's template is the donor's magazine's (the SCAR-L's own, `61840bedd92c473c77021635`), so **its prefab is hung on `mod_magazine` as the game hangs it, before the model's parts, and the model's magazine is hung below it** (`build`'s `hung`); nothing of the donor's magazine is drawn and no hand goes by it. The donor's magazine's five points hang where they are in it, and the edit that moves the magazine's bone (above) carries them with the model's magazine.

**Where its rounds lie** (`probe custom 66f0a5740000000000000044`, in the axes of the weapon's bone at rest): the five points 0.5 cm inside the model's magazine's rear wall, 0.4 cm to either side by turns (the SCAR-L's stagger, for its 5.56), 0.4 to 2.3 cm below its top, within its walls (2.4 to 2.8 cm across). `PROVISIONAL`: the points are the SCAR-L magazine's, and the round drawn is the AKM's 7.62x39 (56 mm), 8 mm longer than the 7.92x33 the model's magazine is cut for: its tip may show past the front of the lips.

**Let go, and in its picture** (`trp_data::custom::magazine_model`; the app's `icons::Worker::model`): the magazine is that object with what is below it, the model's magazine drawn with its own material alone, and its rounds where its points are; its picture posed by its prefab's icon settings, as the SCAR-L magazine's would be. On the ground it draws with the pictures the weapon in hand already put on the card (since 0.65.1: `findings.md`, "The run's first rig shared none of its pictures"). Until 0.65.0 it was the SCAR-L's magazine, drawn from the prefab its template is made from.

**The cases** were already thrown (since 0.54.0, `firing.md`): from the donor's `shellport`, by its own extraction.

**Seen** (`local\shots\`): the flash at frame 706 of `--weapon 66f0a5740000000000000044 --hold auto+fire` (`stg_fire_706_s92.png`, a case flying out to the right in it and in `_712_`), the smoke after the burst (`stg_smoke_1000_s92.png`: the burst ends at frame 901), the magazine checked with its top round (`stg_check2_s92.png`, `--fov 75 --resolution 1920x1080 --tap 300:alt:80 --tap 310:t:5`, frame 450), the magazine let fall in a quick reload (`stg_dropped3_s92.png`, `--view watch --yaw 60 --tap 300:r:3 --tap 312:r:3`), its pictures in the tactical rig (`stg_inventory_s92.png`). **Tested** (`a_weapon_of_ones_own_is_built_on_its_donors_rig`: the jet and smoke below its fireport half a centimetre ahead, the magazine's object first under its slot with the model's magazine and five points below it, nothing of it drawn, every object after its parent, the magazine as a thing with one part and one material); its donor's animator is still followed frame for frame (`a_weapon_of_ones_own_is_worked_by_its_donors_animator`). **Not compared with anything**: the game has no such weapon.

## The magazine's clips (2026-10-24, 0.116.0)

The user, 2026-10-24: "the STG should use the gen 3 40 round magazine animation since it is a longer magazine, and the hand clips when using the original animation." The StG 44 is not the game's, so its magazine's clips are ours to choose: its magazine's template is given `magAnimationIndex` 3 (`custom/stg44.json`'s magazine `set`), the index of the PMAG Gen M3 40 on the donor (the SCAR-L's `scar_16_mag_out_3`, `scar_16_mag_in_3`; `firing.md`, "Which magazine"). The left hand's magazine hold's palms are the donor's on those clips, measured where the magazine is at the well (`a_weapon_of_ones_own_is_worked_by_its_donors_animator`, the donor's palm from its magazine bone): (−8.4, −20.7, −8.6) cm as the old one is drawn out (`RELOAD OUT` 0.30 to 0.40, 12 frames) and (−7.8, −23.3, −7.1) as the new one is seated (`RELOAD IN` 0.46 to 0.80, 55 frames), where the 30-round's had been (−9.9, −16.8, −8.2) and (−9.8, −22.3, −2.8): the hand some 4 cm lower, on the longer magazine. Measured against the donor on the same clips: the hand on the magazine for 76 frames of a reload from empty, at the donor's hold to 0.00 cm; the handle's pull and the rest as before. Looked at (`local\shots\stg_reload_s123.png`: the palm under the magazine's foot as it goes in).

## Not built, and open

- **The donor's catch for the bolt** is never caught, so nothing of this rifle shows one; the donor's hammer, its magazine's catch and its bolt's release have no part of this model on them.
- **The selector's button does not slide.** It turns in place with the donor's lever, which a round button does not show; its 4 mm from side to side are not drawn. (And see `findings.md`: in the simulation the selector's layer does not follow a switch at all yet, for the AKM either.)
- **The left hand's fingers** are the donor's, closed for the donor's handguard, magazine and handle. This rifle's handguard is narrower and rounder.
- **Where a spent case leaves and where the round lies** (`shellport`, `patron_in_weapon`) are the donor's, 0.8 cm from this rifle's port. The cases are thrown from there (since 0.54.0); the chambered round is not drawn.
- **The dust cover** is drawn open, as the model has it; the rear sight's slide is not moved.
- **The pictures of its metal** are drawn as the standard shader takes them. The install's own weapons are approximated another way (`README.md`), and the two were set side by side only by eye.
- **A second weapon of one's own** needs a file like this one; a model whose barrel does not run along -z with y up, or that is one mesh and not parts, needs more than the file (the reader keeps each mesh's connected pieces, which is what such a model's parts would be told apart by).
