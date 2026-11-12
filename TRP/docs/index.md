# Index

Every document and every source file, with one line on what it holds. This file is generated: run `bash tools/index.sh` after adding, moving or renaming a file, and do not edit it by hand.

To find where something lives, search this file for a word (`grep -i "quick bar" docs/index.md`) and open the file it names. To read one section of a long spec, use `bash tools/section.sh <spec> "<heading>"`.

## Documents

- `README.md`: TRP rebuilds the first-person character controller of Escape From Tushonka in Rust, on the Bevy engine.
- `AGENTS.md`: TRP rebuilds Escape From Tushonka's first-person character controller in Rust on Bevy.
- `CHANGELOG.md`: One number covers the whole of TRP.
- `docs/features.md`: What TRP builds, one line each.
- `docs/manual.md`: How to run TRP, what the keys and flags do, and how to look into the install and compare with the game.

## Specs: how the game behaves (`docs/spec/`)

- `ambient-occlusion.md` (11 KB): How the game darkens creases, corners and the ground under things (its graphics setting `SSAO`), and how the app draws it.
- `ballistics.md` (55 KB): What a shot does after it leaves the weapon: where it starts and which way it goes, how it flies, what it meets, what each hit does to it, and what a hit shows and sounds.
- `blind-fire.md` (29 KB): How the character fires its weapon blind, held over cover or out to the right.
- `body-animator.md` (142 KB): How the game animates the player's body, and what the first-person view and the movement take from it.
- `body-hits.md` (64 KB): What a bullet does when it meets a character: the character's hit colliders, what the bullet does at one, the armour it meets, and the damage that reaches health.
- `bolt-action.md` (14 KB): What a rifle whose bolt is worked by hand does that the AKM and the M3 do not: the bolt is worked after each shot, and only once the trigger is let go; and its magazine, which stays in the rifle, is filled a round at a t...
- `custom-weapons.md` (41 KB): How a weapon that is not in the install is made to work here: its model on the rig of one of the install's weapons, worked by that weapon's animator, with the animation edited where the two do not fit.
- `firing.md` (198 KB): How the weapon decides to fire, how fast, and how a reload runs.
- `footsteps.md` (39 KB): What the game plays for the player's own feet and kit as the player moves, stops, turns, jumps, lands, crouches and crawls; how it knows what the feet are on; and how loud it plays each in first person.
- `free-look.md` (11 KB): The camera's own turn while a key is held, apart from the body's and the weapon's, and its way back when the key comes up.
- `gestures.md` (23 KB): The game lets the character make a hand sign (eleven of them: There, Hold, Greet, Flip off, Thumbs up, Thumbs down, Follow me, Rock, Scissors, Paper, OK) or say a phrase in its voice (a command, a reaction, a call of a c...
- `grenades-and-knife.md` (96 KB): What the hands do with a grenade and with a melee weapon, and what a grenade thrown does.
- `hands-on-weapon.md` (24 KB): How the arms come to hold the weapon.
- `health.md` (150 KB): The character's health: its body parts and their hit points, energy and hydration, the effects that hurt it, heal it or change what it can do (bleeding, fractures, pain, tremor, painkillers, hunger and thirst, medicine a...
- `icons.md` (48 KB): How the game makes the picture of an item in its inventory, read from its code on 2026-10-14 (`IconCreatorBase`, `ItemIconCreator`, `ItemIconCache`, `ItemViewFactory`, `PreviewPivot`, `IconShadow`, `ShaderReplacer`, `Wea...
- `inspect-window.md` (45 KB): The window that shows a thing (`InfoWindow`, `ItemSpecificationPanel`, `ItemInfoWindowLabels`;
- `interactions.md` (68 KB): How the character works a door, searches a container, opens a car's trunk and takes a thing up: what it can look at, which animation the left hand makes, whether the character is held for it, and what the thing itself do...
- `inventory.md` (406 KB): What the character carries, where each thing sits, and the screen that shows it.
- `left-shoulder.md` (18 KB): The weapon moved to the left shoulder.
- `lighting.md` (75 KB): How the game lights a level and how its camera brings that light to the screen, read for Factory by day, and how the range takes both.
- `malfunctions.md` (44 KB): What a shot does to the weapon and what the weapon then does: the wear of each shot, the chance of a malfunction and which one comes, what each does to the rounds and the hands, the look that tells its kind and the repai...
- `more-weapons.md` (64 KB): Tarkov's weapons counted, and one weapon built of each kind the AKM, the M3 Super 90, the Five-seveN and the Model 700 do not cover.
- `mounting.md` (59 KB): How a weapon is rested on cover or on its own bipod, and what that changes.
- `movement.md` (153 KB): How the player moves, written from a reading of the game (0.16.9.5, SPT 4.1.6 assembly).
- `optics.md` (55 KB): How a sight fitted to a weapon is chosen, aimed through, zoomed and drawn.
- `outfits.md` (30 KB): A body of S.T.A.L.K.E.R.'s, worn as an item in the armour's slot, drawn in place of the character's own with its own build and moved by Tarkov's animation.
- `particles.md` (51 KB): Unity's particle system (its "Shuriken"), as the game's effects use it and as this work runs it.
- `physics.md` (32 KB): What the game hands to its physics engine, and how that engine is set up.
- `pistol.md` (6 KB): What a pistol does that the AKM does not: its slide stays back on the last round and is let go by a reload; a press during a shot is remembered; it hangs in the holster; and the body and the weapon's motion are told it i...
- `range-targets.md` (8 KB): The reactive targets far down the range, after the hideout's poppers.
- `rangefinder.md` (7 KB): The game's one handheld rangefinder, taken in hand from a quick slot, aimed through to read
- `settings.md` (101 KB): How the game hears its keys and lets them be changed, which of its display settings the app takes, and the tab the app keeps them on.
- `shotgun.md` (23 KB): What a weapon with a magazine that stays in it does that the AKM does not: a shot followed by the pump, two modes with states of their own and a change between them that takes time, a tube filled a shell at a time that c...
- `skills.md` (75 KB): What Tarkov's skills change in the character and how they are earned, where each figure lives, and how far the app has them.
- `sound.md` (57 KB): What the game plays while its screens are worked (opening the inventory, a tab under the mouse, a thing taken up and put down, a window opened and shut), where it keeps those sounds, and how loud it plays them.
- `stamina.md` (47 KB): What the character pays for moving and for holding a weapon up, and what running out does.
- `streaming.md` (13 KB): How much of each picture the game keeps on the graphics card.
- `tactical-devices.md` (31 KB): How the weapon's lights, lasers and combo units and the head light are switched and drawn.
- `third-person.md` (66 KB): How the game poses and moves a character for eyes other than its own: another player's, a bot's, a camera's.
- `underbarrel-launcher.md` (41 KB): The underbarrel grenade launcher, the last of the weapon's smaller parts.
- `vaulting.md` (41 KB): How the character gets over a low obstacle or up onto one.
- `weapon-handling.md` (142 KB): How the weapon and the camera move in first person beyond what the animation clips do: sway, lag, bob, aiming.
- `workbench.md` (61 KB): The range's workbenches and the edit build screen they open.

## Source

### crates/trp-app

- `build.rs` (166): Reads TRP's version from the workspace and makes the exe's icon from the install.

### crates/trp-app/src

- `main.rs` (158): TRP, the Tushonka Runtime Project: the game's first-person controller rebuilt on Bevy, and its range.
- `app_options.rs` (460): The run's data, the view and the app's options.
- `startup_crates.rs` (450): The range's weapon crates, gear cabinets, repair boxes and containers filled.
- `startup_kit.rs` (322): The range's dummies' kit, armed builds, crates of a kind and the tables' loads.
- `views_capture.rs` (241): Switching views, the free camera and the screenshot capture.
- `warmup_log.rs` (408): The warm-up before the first frame, the pacing log and the frames' log.
- `world_setup.rs` (265): The world set up: the main camera, the things shown, the scene.

### crates/trp-app/src/app_assembly

- `mod.rs` (234): The app assembled: its plugins, resources and systems.
- `body.rs` (113): The body's parts, its animator and standing pose, the movement made from them, and the hands' data.
- `bundles_and_skills.rs` (103): The bundles' index, the movement's settings, the experience, the skills and the switches read from them.
- `camera_passes.rs` (67): The camera's passes and the world's light, from the tonemap to the frame limit and the gamma.
- `character_kit.rs` (150): The database loaded, the run's weapons named and the character's kit put together.
- `controller_and_screens.rs` (108): The controller with its health, the battle screen's panels waited for, the pointers and the repair's figures.
- `dummies.rs` (110): The range's dummies: their kits, their looks and their voices.
- `dummy_source.rs` (144): The dummies' colliders, armour, health and characters, and the character's own armour against a blast.
- `effects_and_bullets.rs` (61): The switches of the sounds and the effects as the run begins, the sounds' install, and the bullets.
- `effects_plugins.rs` (52): The plugins of the shots' effects, the hands' sounds, and the sounds' log and tape.
- `engine_plugins.rs` (121): The engine's and the app's plugins, with the resources they begin with.
- `graphics.rs` (108): The screens' reading thread started, and the graphics' settings from textures to ambient occlusion.
- `install_options.rs` (47): The run's start: its version and log, the install found and the options read.
- `item_texts.rs` (80): The install's texts and names, the equipment panel, the inventory's rules and the airdrop crate.
- `level.rs` (88): The range laid out: a level file, its workbenches, the capsule, the doors, the start, and the camera's box.
- `loose_things.rs` (89): The ragdoll, the loose things on the ground, the crates filled to be searched, and what the run begins with.
- `range_things.rs` (207): The range's things: the gun cases, the cabinets, the containers, what lies about and what is put on.
- `render_streaming.rs` (33): The renderer's own systems, the wardrobe, and the pictures' streaming and the frame's timing.
- `renderer.rs` (91): The clip player, the window's updates and the renderer's settings: backend, shader compiler, window, render thread.
- `resources.rs` (149): The app made, and the resources it begins with: the controller, the inventory's state, the game's data.
- `switches.rs` (30): The keys, the options tab's state and the features' switches as the run begins.
- `systems_frame.rs` (45): The frame's systems: the start-up's, and the Update chain from the input to the warm-up.
- `systems_sounds.rs` (70): The sounds read away from the frame and their systems, the hearing, the sharpening, the visor and the magazines' rounds.
- `systems_targets.rs` (104): The systems of the targets, the slung weapons, the watch, the bullets, the workbench and the dummies.
- `world_light.rs` (125): The world's light: Customs' light and clouds, the camera's filter and sharpening, and the shading chosen.
- `worn.rs` (118): What is worn as the run begins, the suits, the slung weapons' bones and the wrist watch.

### crates/trp-app/src/assets

- `mod.rs` (9): Reading the install into the engine: meshes, pictures, streaming and reads off the frame.
- `background.rs` (87): Reads made away from the frame: taken on the frame they come, or waited for where they are first wanted.
- `bc7_maps.rs` (129): The install's normal and gloss pictures packed as BC7, made BC5 of the two channels the material reads.
- `effects_reads.rs` (117): The effects read away from the frame.

### crates/trp-app/src/assets/lod_view

- `mod.rs` (144): Levels of detail by distance on what is drawn, by TRP's own rule.
- `dither_shader.rs` (100): The dither shader and its patch into the mesh functions.
- `levels.rs` (393): The level-of-detail specs, pieces and bands, and the levels picked for each view.
- `tests.rs` (63): Tests of the levels of detail by distance and TRP's own rule.

### crates/trp-app/src/assets/render_data

- `mod.rs` (30): Turning extracted Unity objects into Bevy meshes, images and materials.
- `converted.rs` (230): Converted meshes and materials, and STALKER pictures.
- `mesh_build.rs` (348): Meshes unskinned, posed and built, and images built from textures.
- `pictures_added.rs` (219): The prepared looks' pictures counted, marked and added, and shared pictures.
- `prepared_looks.rs` (342): Prepared materials, the game's figures, and the prepared looks' meshes and materials.

### crates/trp-app/src/assets/streaming

- `mod.rs` (51): The models' pictures by their levels of detail, as the game's Unity keeps them.
- `mip_state.rs` (234): Mip streaming's state, new pictures taken, and the card watched.
- `stream_frame.rs` (317): Made pictures taken, and the streaming decided each frame.
- `tests.rs` (52): Tests of the models' pictures by their levels of detail and the mip streaming.
- `textures.rs` (251): The texture settings, sources added, and streamed textures' levels.

### crates/trp-app/src/assets/worn_reads

- `mod.rs` (130): The worn things read when they are first worn.
- `prepare_pieces.rs` (135): Worn pieces prepared from the install on a worker.
- `reading.rs` (292): What is worn now, the worn pieces read in, and the next one asked.

### crates/trp-app/src/audio

- `mod.rs` (13): Everything heard.
- `blast_sound.rs` (134): A grenade's blast's sound.
- `hearing.rs` (97): A helmet's cut of what is heard.
- `impact_sound.rs` (178): A bullet's hit heard.
- `landing_sound.rs` (155): A weapon heard as it lands.
- `thing_sound.rs` (252): The things' own sounds.
- `tinnitus.rs` (141): A contusion's tinnitus.

### crates/trp-app/src/audio/hands_sound

- `mod.rs` (140): The sounds of what is in the hands.
- `playing.rs` (198): The hands' sounds played each frame.
- `reading.rs` (179): The rig's sounds read, and the aiming banks read and chosen from.
- `tests.rs` (30): Tests of the sounds of what is in the hands.

### crates/trp-app/src/audio/shot_sound

- `mod.rs` (479): The shots' sounds.
- `tests.rs` (34): Tests of the shots' sounds.

### crates/trp-app/src/audio/step_sound

- `mod.rs` (194): The sounds of the player's own moving.
- `playing.rs` (332): Steps played for the character and the dummies, and bodies' falls.

### crates/trp-app/src/audio/ui_sound

- `mod.rs` (70): The interface's and the things' sounds, as the game's `GUISounds` plays them.
- `kinds.rs` (159): What a sound is: the told and heard, the screen's sounds and their ways.
- `mixing.rs` (285): The mix, its gains, the tape, and the mixing source.
- `playing.rs` (210): Sounds read and their clips taken, the mixer started, and sounds played.
- `samples.rs` (243): Levels, samples, pitch, and the voices playing.
- `tests.rs` (232): Tests of the interface's and the things' sounds.

### crates/trp-app/src/audio/voice_sound

- `mod.rs` (116): The character's voice and the health's own sounds.
- `choosing.rs` (208): Voices read, one chosen and kept, and its preview played.
- `speaking.rs` (207): Phrases spoken in the chosen voice.

### crates/trp-app/src/cli

- `mod.rs` (145): Parsing the command line into the app's options.
- `character_args.rs` (120): The command line's arguments of the character: its kit, skills, mastering and the weapon's state.
- `display_args.rs` (137): The command line's arguments of the world, the graphics and the run's outputs: captures, logs and the window.
- `input_args.rs` (155): The command line's arguments of scripted input and of the view: keys, mouse, drags, the camera and the look.

### crates/trp-app/src/effects

- `mod.rs` (12): Effects in the world and on the screen.
- `blast_view.rs` (244): A grenade's blast on the character.
- `blood_view.rs` (407): Blood on the screen.
- `distort_view.rs` (299): The camera's distortion.
- `offscreen_dust.rs` (268): The effects' smoke and dust drawn off the frame at a smaller size, the options' DUST RESOLUTION.

### crates/trp-app/src/effects/contusion_view

- `mod.rs` (70): A contusion's screen effects.
- `contusion_state.rs` (406): The contusion and eye-burn screens, the hits received, and their update each frame.
- `passes.rs` (330): The contusion pipeline, its passes and the view nodes that run them.

### crates/trp-app/src/effects/decal_view

- `mod.rs` (116): The game's deferred decals as the app lays them.
- `holes.rs` (419): Bullet holes and blood: their pictures read, prepared and laid.
- `scorches_patch.rs` (234): Scorches prepared, decals laid and tagged by stencil, and the decal shader patched.

### crates/trp-app/src/effects/explosion_view

- `mod.rs` (120): A grenade's explosion.
- `camera_view.rs` (222): What the camera draws of the explosions each frame.
- `effects_pass.rs` (236): The render pipeline and the view node that draws the effects.
- `exploding.rs` (341): Exploding, placing the lights and composing each frame's explosions.
- `looks.rs` (193): The effects' light and each material's look.
- `preparing.rs` (320): Readying systems, preparing explosions, emitting entries and impacts.
- `ready_parts.rs` (312): The uniform, the readied systems, trails and lights, and the explosions' state.

### crates/trp-app/src/effects/glass_view

- `mod.rs` (309): The see-through glass with the options' `SEE-THROUGH GLASS`.
- `glass_drawn.rs` (115): The glass drawn: its twins readied, swapped and followed, and the cameras marked.
- `undither_pass.rs` (92): The undither pipeline, its parts and the view node.

### crates/trp-app/src/effects/particles_view

- `mod.rs` (52): The particle system's effects.
- `fume_draw.rs` (275): The fumes' light, uniform and pictures, prepared and drawn each frame.
- `haze_draw.rs` (291): The hazes' uniform, batches and systems, prepared and drawn each frame.
- `spark_draw.rs` (152): The sparks' uniform, prepared and drawn each frame.

### crates/trp-app/src/effects/visor_view

- `mod.rs` (67): The face shield's mask over the first person's view.
- `visor_pass.rs` (331): The visor and crack pipelines, their parts prepared, and the view node.
- `visor_state.rs` (210): The visor and crack screens, their images and state, and the visor's frame.

### crates/trp-app/src/panels

- `mod.rs` (7): The in-raid panels and prompts.
- `action_view.rs` (266): The battle screen's interaction panel and pointer, drawn as the game's with `TARKOV UI`.
- `using_view.rs` (164): The panel shown while the character examines a thing, with its name and a filling bar.

### crates/trp-app/src/panels/battle_view

- `mod.rs` (72): The battle screen's stance panel, health panel and quick bar, drawn and faded as the game's.
- `battle_state.rs` (233): The battle view's state, the quick bar's glow and the picker's cells.
- `drawn_parts.rs` (466): The battle view drawn: the quick bar and picker, health followed, fading.
- `modes_fader.rs` (242): What the battle view shows, its modes and components, and the fader.
- `stance.rs` (359): The stance followed and drawn.
- `tests.rs` (148): Tests of the battle screen's panels.

### crates/trp-app/src/panels/hud

- `mod.rs` (128): The app's own stance, speed and stamina display, and the debug panel at the top right.
- `debug_guide.rs` (229): The top panel spawned, hidden over screens, and the debug and guide panels.
- `meters.rs` (207): The meters spawned and shown, and the frame rate.

### crates/trp-app/src/panels/sight_panels

- `mod.rs` (439): The battle screen's ammunition and optic panels at the lower right, faded in and out as the game's.
- `tests.rs` (71): Tests of the battle screen's ammunition and optic panels.

### crates/trp-app/src/platform

- `mod.rs` (16): The app's own plumbing: window, graphics backend, frame limit, logs, first-start setup and key bindings.
- `backend.rs` (115): Which graphics backend the renderer draws through, Vulkan or DirectX 12.
- `convert.rs` (62): Unity space to Bevy space.
- `cursor.rs` (352): The mouse and the window.
- `debug_menu.rs` (405): The debug panel's commands, worked by its buttons with the mouse freed while it is open.
- `frame_limit.rs` (100): The game's frame rate limit in a raid.
- `memory.rs` (192): What the run holds in memory and in the world, noted in its log.
- `options_store.rs` (56): One key of the options file read or kept, with everything else in the file left as it was.
- `park.rs` (162): Takes what is hidden for long out of the engine's work until it is shown again.
- `render_timing.rs` (397): Where the renderer's time goes in a slow frame.
- `run_log.rs` (492): The run's log, the check of the install's versions and a crash's report.
- `setup.rs` (171): The first start's questions, asked in the console the app runs in, of where the player's games are.
- `unready.rs` (113): A check of what is meant to be drawn and cannot be, frame by frame.
- `window_icon.rs` (57): The window's icon, the one the build puts into `trp.exe`.

### crates/trp-app/src/platform/keys

- `mod.rs` (29): The game's key bindings as the install has them.
- `bound_keys.rs` (219): The bindings, the keys heard each frame and the scripted mouse.
- `controls.rs` (150): The install's controls, the mouse's scale and the keys' words.
- `key_tables.rs` (259): The keys, the tables of keys as read and as built, and taps.
- `tests.rs` (40): Tests of the game's key bindings as the install has them.

### crates/trp-app/src/player

- `mod.rs` (16): The first-person character: the controller, the rig, the body and what it wears and earns.
- `animation.rs` (273): The rig in the hands as it is drawn.
- `arms.rs` (283): Holding the weapon: each arm is solved onto the weapon as the game solves it.
- `feet_down.rs` (95): The options' `GROUNDED FEET`.
- `hands_view.rs` (128): Puts the weapon-motion simulation's results on the rig: the weapon bone and the camera.
- `skills.rs` (333): The character's skills.
- `watch_view.rs` (220): The watch on the first-person hands' wrist, showing the computer's own time.

### crates/trp-app/src/player/body_view

- `mod.rs` (154): The body as drawn: its clothing skinned to a skeleton that the simulation's body animator poses.
- `head_fitting.rs` (406): Fitting head things, hoods and armbands, and turning helmet hinges.
- `head_models.rs` (421): Head models kept, hung and fitted, and read models hung on the body.
- `loading.rs` (472): Loading the body's parts from the install.
- `pieces.rs` (365): The body's parts, suits, outfit pieces, heads and worn pieces.
- `posing.rs` (204): Collarbones seen and kept, the outfit seen and posed, the upper body shown.
- `showing.rs` (235): Which body and suit is drawn, and the outfit, torso, worn things and head parts shown.
- `spawning.rs` (500): Spawning the body with its rig.

### crates/trp-app/src/player/clothing

- `mod.rs` (116): The clothing chosen on the OVERALL tab, every side's suits for any character, each loaded when chosen.
- `prepare_keep.rs` (242): Clothing's parts prepared and loaded, the choice kept, and the joints found.
- `wearing.rs` (238): Clothing put on the body.

### crates/trp-app/src/player/controller

- `mod.rs` (394): Drives the simulation from the keyboard and mouse and keeps what is to be drawn of it.
- `input.rs` (192): Gathering the frame's input, and the hands' log.
- `pose_parts.rs` (332): The shown pose, body pose, springs, mounts and the pending changes the controller keeps.
- `rig_blend_tests.rs` (29): Tests of the blend between two poses of the rig.
- `showing.rs` (356): The controller made, and what it shows of the rig and the body.

### crates/trp-app/src/player/controller/stepping

- `mod.rs` (215): Stepping the controller one frame.
- `firearm_frame.rs` (352): The firearm's frame: its trigger and keys, then its logic stepped and what it did told on.
- `hands_ready.rs` (127): The hands made ready for the frame: their settings, health and pose, the aim, and the springs' physics steps.
- `held_items.rs` (214): The other things the hands may hold, stepped: empty hands, an item used, a grenade, the rangefinder and the knife.
- `input.rs` (150): The frame's input to the movement: what the thing in hand says of itself, and the keys as the movement takes them.
- `keys.rs` (211): The frame's keys as a script or a played run gives them, and the weapon's keys heard before the body moves.
- `movement_told.rs` (111): The movement's frame, and what it did told on: blind fire, the mount and its sounds, health, the range's things.
- `placement.rs` (200): The rig placed for the frame: its pose read, the weapon and the camera where it leaves them, and the overlap measured.

### crates/trp-app/src/player/death

- `mod.rs` (80): The character's death as the game shows it.
- `corpse_state.rs` (178): The corpse, its outfit and ragdoll, the death state and the physics clock.
- `dying.rs` (337): Dying, and the corpse followed afterwards.
- `fade_again.rs` (168): The fade to black and beginning the run again.

### crates/trp-app/src/player/experience

- `mod.rs` (493): What the character earns by what it does, from every source of experience the game has.
- `tests.rs` (29): Tests of what the character earns by what it does.

### crates/trp-app/src/player/gestures

- `mod.rs` (169): The hand signs and the voice's phrases.
- `binds.rs` (195): The menu's picks, the Gestures state and the binds kept.
- `listen_layout.rs` (372): The menu's frames and layout, and the keys heard while it is open.
- `menu_drawn.rs` (303): The prompt and the menu drawn.
- `phrase.rs` (190): The voice's phrase triggers, their names and priorities, and which one a moment of play sets off.
- `pictures.rs` (222): The menu's pictures read, the situations and a phrase played.
- `tests.rs` (98): Tests of the hand signs and the voice's phrases.

### crates/trp-app/src/player/slung_view

- `mod.rs` (60): Weapons not in hand, drawn where the game hangs them on the body.
- `hanging.rs` (357): Keeping slung things hung, letting go, and hanging the character's and others' things.
- `slung_state.rs` (284): The bodies things hang on, the slung state and the bones wanted.
- `tests.rs` (212): Tests of what is hung on a body and where.

### crates/trp-app/src/player/statistics

- `mod.rs` (384): The experience a thing taken from the world gives, as the battle screen tells it.
- `tests.rs` (167): Tests of the experience a thing taken from the world gives.

### crates/trp-app/src/rendering

- `mod.rs` (15): How a frame is lit and finished: light, sky, shading and the camera's effects.
- `clouds_view.rs` (410): Customs' clouds with the options' `CUSTOMS CLOUDS` on, under `CUSTOMS LIGHT`.
- `color_filter_view.rs` (206): The camera's own colour filter with the options' `CAMERA COLOR FILTER` on, under `CUSTOMS LIGHT`.
- `game_surface.rs` (419): The game's own surface shader, for the two views that draw an item alone under lights of their own.
- `hbao_view.rs` (364): The game's ambient occlusion with the options' `AMBIENT OCCLUSION` on.
- `highlight_view.rs` (194): The game's ambient highlight with the options' `AMBIENT HIGHLIGHT` on, under `CUSTOMS LIGHT`.
- `prism_view.rs` (326): The camera's PRISM effect with the options' `CUSTOMS LIGHT` on.
- `scattering_view.rs` (205): The sky's scattering over the scene with the options' `SKY SCATTERING` on, under `CUSTOMS LIGHT`.
- `shading.rs` (137): Tarkov's surface shading, with the options' `TARKOV SHADING`.
- `shadow_only.rs` (87): A mesh drawn as its shadow alone, as Unity draws a renderer whose shadow casting is "shadows only".
- `shadows.rs` (84): The sun's shadows as the game's own quality presets set them.
- `sharpen_view.rs` (225): The camera's sharpening and desaturation, the whole pass under a switch of its own.
- `tod_sky_view.rs` (187): A level's time-of-day sky about the camera.

### crates/trp-app/src/rendering/lighting

- `mod.rs` (42): The range lit as Customs in the recorded morning.
- `hall_light.rs` (357): The hall's light probe and lamps, and the range's own sun.
- `level_lighting.rs` (393): The level's lighting read: exposure, sharpening, scattering, sky and harmonics.
- `tests.rs` (39): Tests of the range lit as Customs in the recorded morning.

### crates/trp-app/src/screens

- `mod.rs` (23): The inventory's tabs and windows.
- `ammo_picker.rs` (212): The magazine picker: with R held, the wheel chooses what the weapon is reloaded with.
- `boot_shadow.rs` (165): The shadow at the OVERALL figure's feet.
- `examine_view.rs` (199): Examining things: what the character knows, an examine begun and ended with the game's notice.
- `figure_surface.rs` (255): The OVERALL tab's own figure's surface, lit as the game's menu figure is.
- `item_menu_view.rs` (421): The menu of a right click on a thing in the inventory, with its lines and their own lines.
- `overall_view.rs` (490): The inventory's OVERALL tab, with its statistics and the clothing dropdowns that change the character's look.
- `preview_outline.rs` (201): The edit build screen's outline of the part under the mouse.
- `preview_tonemap.rs` (173): The photographic tonemap of the inspect window's preview camera.
- `repair_view.rs` (475): Repairing at the workbench with a repair kit, in the game's repair window.
- `search_view.rs` (317): Searching containers and bodies, as the game's searchable views do it.
- `split_view.rs` (389): The window that asks how many of a stack to move, drawn as the game draws it.

### crates/trp-app/src/screens/edit_build

- `mod.rs` (261): The edit build screen, opened at the gun bench for a weapon and at the armor bench for gear.
- `clicks.rs` (261): Clicks on the screen and its parts, and the tooltip.
- `drawing.rs` (352): Drawing the screen, and the app's stats panel.
- `outline_bones.rs` (416): The outline, the bones and gear points, and the slots placed and prepared.
- `preview_model.rs` (246): The preview's zoom, its model put in place and its lights.
- `preview_view.rs` (357): Turning the preview and drawing it each frame.
- `screen_parts.rs` (357): The screen's components, and opening, keys, releasing and shutting it.
- `slot_views.rs` (438): The slots' lines, bones, views and dropdowns, in the game's look and the app's.
- `tests.rs` (82): Tests of the edit build screen.

### crates/trp-app/src/screens/figure_view

- `mod.rs` (154): The character's own model, drawn behind the gear page's slots and on the OVERALL tab.
- `builder.rs` (452): The figure's skeleton, the Hangs trait and the builder that makes figures.
- `camera_draw.rs` (370): The figure's camera, lights, picks and drawing.
- `figure_state.rs` (211): The figure's twin, images, OVERALL figure, held weapon and shots.
- `live_weapon.rs` (427): The weapon on the figure read, and the figure kept live and posed.
- `tests.rs` (131): Tests of the inventory's figure.

### crates/trp-app/src/screens/health_view

- `mod.rs` (314): Health on the screen and in the world: the HEALTH tab, the death screen and the barbed wire.
- `damage.rs` (271): Damage keys, armour slows, the death screen and the barbed wire.
- `eb_stats.rs` (287): The edit build screen's stats bars built and drawn.
- `grid_window.rs` (183): The grid window built and drawn, with its search loader.
- `health_panel.rs` (241): The HEALTH panel filled from the character's state.
- `info_layout.rs` (425): The inspect window's words, places and scroll, and its parts built.
- `info_pieces.rs` (319): The inspect window's mastering, buttons and slots.
- `info_table.rs` (288): The inspect window's rows, and the window drawn.
- `item_grounds.rs` (431): Binds, item grounds and looks, the pictures made and the version line.
- `looks.rs` (394): HealthLook and the other looks read from the game's scene, with their paths.
- `menu.rs` (183): The right-click menu's looks and layout.
- `nodes.rs` (456): Nine-slices, marks, frames and the nodes drawn.
- `overall.rs` (248): The OVERALL tab's texts and drawing.
- `overall_tests.rs` (52): Tests of the OVERALL tab's texts.
- `own_panel.rs` (284): The app's own health panel and its figures.
- `panel_pieces.rs` (413): The panel's pieces, the drop's glow, the effects' icons and tooltips.
- `panels_read.rs` (212): The edit build and OVERALL panels, the picture reader and what is shown.
- `pixels.rs` (235): Pixels, masked fills, colour spaces, the drawn pieces and the weight limits.
- `quick_containers.rs` (356): The quick bar's views, the tabs' places, the containers and special slots.
- `repair_window.rs` (294): The repair window built and drawn.
- `row_search.rs` (239): The rows' search panels, the dogtag slot and the stash's search fills.
- `scene_fonts.rs` (257): The scene's fonts, keys, sprites and finds by content.
- `scene_read.rs` (298): Reading the scene, and the battle part split, taken and rejoined.
- `sprites_tips.rs` (282): Sprites drawn, stretching, mastering looks and tooltip looks.
- `stash.rs` (292): The stash's places and drawing, the search tint and the complex view.
- `strip_tabs.rs` (205): The strip, the gear, the foot, the border and the tab bar.
- `tag_window.rs` (139): The tag window's colours, places and drawing.
- `tests.rs` (114): Tests of the game's screens as read and drawn.
- `tmp_texts.rs` (351): TextMeshPro's texts: effects, copies, rich spans and drawing.
- `view_looks.rs` (250): The item view's, spinner's, wish's and tag plate's looks.

### crates/trp-app/src/screens/health_view/screens_read

- `mod.rs` (85): Reading the game's screens into HealthLook.
- `fonts_clips.rs` (95): The screens' fonts, the tag window's asked sizes, and the panels' animation clips.
- `inventory_panels.rs` (150): The inventory's own panels read: the HEALTH panel's parts, the containers, the crate, the quick bar and a thing's views.
- `overall_edit_build.rs` (110): The OVERALL tab and the edit build screen read.
- `sprites_read.rs` (220): The pictures the screens' scripts put on them, and the repair, split and tooltip windows read.
- `windows_read.rs` (163): The windows read: a thing's inspect window with its rows, a container's grids, a tag's and the right-click menus.

### crates/trp-app/src/screens/icons

- `mod.rs` (140): The inventory's pictures, drawn from each item's own model as the game makes them and kept on disk.
- `asking.rs` (205): Icons received, asked for and reported.
- `drawing.rs` (341): Preparing and drawing icons, and the pictures captured.
- `keys.rs` (286): The Icons resource, the rounds drawn, and the pictures' keys and signatures.
- `model_framing.rs` (282): Models for rounds, grenades and STALKER items, and the camera's framing.
- `tests.rs` (319): Tests of the inventory's pictures and their keys.
- `worker.rs` (305): The icon worker: its models, the switched parts and their fitting.

### crates/trp-app/src/screens/info_preview

- `mod.rs` (196): The 3D model in the window that shows a thing, the same model as the workbench's.
- `previewing.rs` (313): The inspect window's preview turned and drawn.

### crates/trp-app/src/screens/inventory_view

- `mod.rs` (314): The inventory screen: its tabs, the gear's slots, the containers' grids and what is open beside them.
- `acting.rs` (279): Acting on things, refusals and spawning the screen's nodes.
- `asking.rs` (440): The asking window's state and how a press on it is handled.
- `blinking.rs` (379): Pictures filled, blinking, taken things dimmed, and slot and grid boxes drawn.
- `carried.rs` (390): The carried things' state and how each thing is named and described.
- `components.rs` (446): The screen's components, drop marks, the held thing and click state.
- `context_menu.rs` (325): The right-click menu's lines, clicks and move sounds.
- `game_item_view.rs` (464): The game's own item view drawn for a thing.
- `gear.rs` (454): The GEAR page.
- `hand_keys.rs` (277): The hands' moves followed and the goggles, launcher, unload and chamber keys.
- `hand_takes.rs` (406): Taking things into the hands, from empty hands or another thing.
- `hands.rs` (325): The quick keys and the hands' state with its chambers.
- `info_parts.rs` (296): The inspect window's parts, its tips and lock tips.
- `item_view.rs` (393): Item views, their icons, values and menu texts.
- `layout.rs` (197): The screen's measures, colours, tabs and panels.
- `loot_panel.rs` (462): Pictures, the reach, bodies' panels and lists, and the loot's scrolling.
- `magazine_work.rs` (295): Slot keys, named builds, round times and the magazines' loading work.
- `pointing.rs` (313): What lies under the mouse, foresight, related things lit and item tooltips.
- `quick_bar.rs` (245): The quick bar and the pictures and values of its slots.
- `reach.rs` (375): The reach, the screen shut and the carried weight.
- `rebuilding.rs` (284): Rebuilding the screen after a change, and scripted drags.
- `scrolling.rs` (466): The game's scroll views and the inspect window's scrolling texts.
- `small_windows.rs` (409): The repair and tag windows, the asking box and the stash's fills.
- `spinners.rs` (370): Unknown and unsearched things, the examine loader and the spinners.
- `tabs.rs` (234): The game's tabs, the opening tween and the screen's sounds.
- `toggle.rs` (491): Showing the ground and opening and shutting the screen.
- `using.rs` (189): Using a thing, putting it down, unloading and fitting magazines.
- `window_frames.rs` (448): Windows stretched and fitted, and each window's box built.
- `window_moves.rs` (214): Windows handled each frame, bench asks and the published drag.
- `windows_open.rs` (363): The windows' state, and finding, opening, raising and shutting them, bodies included.

### crates/trp-app/src/screens/inventory_view/dragging

- `mod.rs` (285): Dragging a thing between grids, slots and windows.
- `letting_go.rs` (142): A thing let go: used on a part of the HEALTH tab, asked how many of, loaded, mended or moved.
- `menu_clicks.rs` (212): Before a thing is taken: the right-click menu's lines, a window's buttons and the clicks on a thing.

### crates/trp-app/src/screens/inventory_view/settling

- `mod.rs` (246): Settling the inventory's changes each frame.
- `before.rs` (86): What is known and asked before the hands settle: checks, rigs to make, a grenade thrown, what health said of items.
- `chambers.rs` (298): The chambers settled: a load given up, the chambered round, the launcher's and chambers of their own.
- `hands_turn.rs` (341): The hands' turn: the first weapon taken as the character is made, an item in use, and a weapon in hand.
- `magazines.rs` (190): The magazines settled: a tube's rounds, reloads' exchanges, the next magazine and its build, and the fitted one changed.
- `rounds.rs` (141): Rounds thrown into the world from the weapon, and the weapon in hand's fold and shots.
- `told.rs` (100): What the hands hold now, told to the workbench, the blinking boxes, the hands and the battle bar.

### crates/trp-app/src/screens/inventory_view/tests

- `mod.rs` (103): Tests of the inventory screen, with the carried things they share.
- `moves_hands.rs` (190): Tests of moves and their sounds, clicks, the hands' takes, chambers and split stacks.
- `naming_looks.rs` (193): Tests of item views, blinking, tooltips, refusals and the names of slots and weapons.
- `sliders.rs` (119): Tests of the asking window's sliders, stops and stepped knob.
- `windows_scroll.rs` (179): Tests of windows grown, kept and stretched, the loot's rows, cells and scroll bars.

### crates/trp-app/src/screens/mastery_view

- `mod.rs` (65): The SKILLS tab's MASTERING page, built as the game's.
- `look_rows.rs` (177): The look read from the install, and the rows listed.
- `mouse.rs` (317): The page's state, the mouse and the keys.
- `rows_drawn.rs` (463): The rows, their pictures and views, and the tooltip drawn.
- `tests.rs` (48): Tests of the MASTERING page.

### crates/trp-app/src/screens/options_view

- `mod.rs` (99): The options tab, the last of the inventory screen's tabs: the keys, the display and the systems' switches.
- `applying.rs` (358): Slides, scrolling, hovering, syncing and applying the options.
- `features.rs` (235): The features' switches as one struct.
- `fields.rs` (350): Each field of the options alone, with its key and slot.
- `look.rs` (279): The page's pages, colours, measures, fonts and components.
- `options_state.rs` (406): The options' state and how it starts.
- `page_controls.rs` (157): The controls page and its cells.
- `page_features.rs` (173): The HUD and FEATURES pages' bodies, with the switches' rows.
- `page_frame.rs` (137): The left column, the page's head and the foot with APPLY.
- `page_graphics.rs` (277): The graphics and sound pages, the voice list and the stepped settings.
- `page_skills.rs` (86): The skills page and its lines.
- `rebuild_clicks.rs` (325): Rebuilding the page and handling its clicks.
- `saving.rs` (221): The options' file, saving and loading, the kept name.
- `slides.rs` (349): Slides, cells, notices, and the state's slider, waiting and naming.
- `switches.rs` (424): The FEATURES page's switches, their sections and what each requires.
- `tests.rs` (328): Tests of the options tab and its file.
- `typing.rs` (278): Resetting a page, typing names and searches, and listening for a key.
- `values.rs` (245): The options' values: mode, graphics, mouse, and the features' defaults.
- `widgets.rs` (277): The parts drawn: buttons, boxes, arrows, bars, rows and sections.

### crates/trp-app/src/screens/skills_view

- `mod.rs` (119): The inventory's SKILLS tab, showing which skills take effect here.
- `drawing.rs` (285): The page's layers and texts drawn each frame.
- `look.rs` (190): The skills page's look read from the install, and effectiveness.
- `mouse.rs` (459): The page's state, its gradual fill, scrolling and the mouse heard.
- `page_drawn.rs` (319): The skills and mastering views built, with their arrows and rates.
- `skill_words.rs` (272): What a skill does here, its words, levels and the experience bar.
- `tests.rs` (137): Tests of the SKILLS tab.
- `tips.rs` (197): The skill's tooltip and the app's own list.

### crates/trp-app/src/screens/text_field

- `mod.rs` (317): A text field as the game's are: what it says, its caret and its selection, and what each key and press does to them.
- `placed.rs` (95): Where a field's letters lie, what of them is under the mouse, and its selection and caret drawn.
- `tests.rs` (296): Tests of the text field: every key, the presses and the drag, and the letter under the mouse.

### crates/trp-app/src/ui

- `mod.rs` (9): What the screens share: tooltips, fades, blur, gamma, fills and layout.
- `gamma_ui.rs` (355): The screens composed in gamma space, as the game's are.
- `radial_fill.rs` (83): A picture filled radially from the bottom, clockwise, as the game's progress spinner is.
- `screen_blur.rs` (327): The world blurred behind the inventory, and a hit's blur, as the game's camera draws them.
- `screen_fade.rs` (217): The game's screens faded in and out softly as they are shown and shut.
- `ui_layout.rs` (68): The interface laid out only on a frame where something it is laid out from has changed.

### crates/trp-app/src/ui/tooltip

- `mod.rs` (205): The game's two tooltips, the simple one and the one of lines, which the screens share.
- `tests.rs` (91): Tests of the tooltips.
- `tip_draw.rs` (311): Simple and lined tips made, placed, hidden and drawn.
- `tip_layout.rs` (205): The tip's corner, runs, marks, measures, ground and words.

### crates/trp-app/src/ui/ui_dump

- `mod.rs` (104): The app's interface written out frame by frame in the lines the game's recorder writes.
- `elements.rs` (249): Each element's words colour, box, ink, path and font.
- `frame_write.rs` (326): The dump's frames, figures, screens and looks, and one frame written.
- `recorder_format.rs` (90): Numbers and strings written as the recorder writes them.
- `tests.rs` (44): Tests of the interface's dump.

### crates/trp-app/src/weapons

- `mod.rs` (16): The weapon as drawn: spawning, attachments, optics, devices, heat, cases and bullets.
- `flare_view.rs` (312): The tactical lights' lens flares.
- `heat_view.rs` (482): The weapon's parts glowing with its heat.
- `kept_settings.rs` (111): A weapon's settings kept on its items: its fire mode, the sight it aims by and each sight's scope, mode, zeroing and zoom.
- `knife.rs` (262): A knife's blade.
- `rangefinder_view.rs` (164): The handheld rangefinder's readout.
- `tracer_view.rs` (149): Tracer rounds' traces on the range.

### crates/trp-app/src/weapons/bullets

- `mod.rs` (165): The shots' bullets on the range.
- `flight.rs` (382): The range as the bullets' world, and bullets flown each frame.

### crates/trp-app/src/weapons/cases

- `mod.rs` (136): Spent cases.
- `case_sounds.rs` (178): Case sounds read and played, and cases let go and launched.
- `flying.rs` (229): Cases flown each frame and spawned.

### crates/trp-app/src/weapons/magazine_rounds

- `mod.rs` (34): A magazine's rounds drawn in its model.
- `chamber.rs` (270): The chamber's kinds kept, its point found, the chambered round shown and rounds spawned.
- `magazine_shown.rs` (217): The magazine's points found and its rounds shown.

### crates/trp-app/src/weapons/muzzle_view

- `mod.rs` (106): A weapon's muzzle flash.
- `flash.rs` (154): Fired muzzles, their flash, shots and light.
- `jet_data.rs` (296): The smoke and jet uniforms, meshes and screens, made each frame.
- `nodes.rs` (474): Buffers prepared, and the nodes that draw jets, lasers, flares, tracers, sparks and fumes.
- `pipeline.rs` (367): The jet render pipeline and its initialisation.

### crates/trp-app/src/weapons/optic_view

- `mod.rs` (106): A scope's picture-in-picture, as the game draws it.
- `culling.rs` (73): The skinned pieces the engine never culls, left out of the scope's view where it cannot see them.
- `materials.rs` (140): The reticle, lens and collimator materials.
- `optic_camera.rs` (359): The optic's camera spawned and placed, the lenses dressed and the sights turned.
- `reticle.rs` (93): The reticle's mesh and its showing.
- `uniforms.rs` (41): The figures handed to the shaders of a scope's lens, its reticle and a collimator.

### crates/trp-app/src/weapons/tactical_view

- `mod.rs` (172): Tactical devices in the app: their keys, their lights and their lasers.
- `aiming.rs` (432): Lamps placed, the kept turn, and lasers aimed.
- `devices_shown.rs` (317): Devices shown on the weapon, slung, on the figure and on the dummies.
- `laser_lamps.rs` (227): The laser's uniform and screen, the tactical keys and the lamps spawned.

### crates/trp-app/src/weapons/weapon

- `mod.rs` (131): Spawning an assembled weapon as an entity tree.
- `hands_found.rs` (275): The hands for a weapon, an item or nothing, and their clips.
- `hands_rig.rs` (313): The hands' rig, and the rigs stowed for each weapon.
- `nodes.rs` (213): Trees and nodes spawned, with their glows, colliders and skins.
- `preparing.rs` (316): Preparing rigs ahead and freeing them.
- `rig_builder.rs` (403): Uploads watched, the rig builder, the preparing worker and the looks.
- `rig_frame.rs` (303): Muzzle parts, shell ports, and the rig's frame and pose.
- `weapon_spawn.rs` (317): Spawning a weapon, and its sounds attached.

### crates/trp-app/src/weapons/wear

- `mod.rs` (68): The weapon in hand's wear, malfunctions and heat in the app.
- `feedback.rs` (391): Notices and malfunctions shown, the feedback given, and the wear kept.
- `notice_looks.rs` (241): The notices' timing and rows, the glow material and the looks read.
- `notices_drawn.rs` (255): Notices shown and spawned, and the glow shown.
- `tests.rs` (65): Tests of the weapon in hand's wear, malfunctions and heat.

### crates/trp-app/src/world

- `mod.rs` (12): The test range and what stands on it.
- `dummy_voice.rs` (482): The dummies' voices.
- `grid.rs` (94): Grid lines on the test range's ground and boxes, so that distance, speed and height can be read off.
- `poppers.rs` (495): The reactive targets.
- `range_day.rs` (134): The range's own light by the hour, set in the app at any time.
- `shield_marks.rs` (79): A face shield's marks on the range.
- `workbench.rs` (200): The benches on the range, two tables of the hideout's level 3 bench.

### crates/trp-app/src/world/dummy

- `mod.rs` (78): The dummies on the range.
- `crowd_layout.rs` (255): Where dummies stand, the crowd, their build, lights and look.
- `dummy_state.rs` (401): The Dummy and Dummies types, their placement, hits and numbering.
- `making.rs` (232): Making the dummies and keeping their worn things and armour.
- `placing.rs` (444): Placing dummies, standing in the way, blasts on them and their falls.
- `puppets.rs` (249): The dummies' rigs, puppet makers, sources, colliders, falls and grips.
- `queries.rs` (455): What the dummies answer: hearing, rays, takes and blows, and their colliders.
- `running.rs` (413): Running the dummies each frame: what is seen and how they are posed.
- `upkeep.rs` (279): Blows, wear kept, weapons cleared, and dummies shown, healed and reweighed.

### crates/trp-app/src/world/dummy/tests

- `mod.rs` (36): Tests of the dummies on the range, with their shared helpers.
- `blows.rs` (153): Tests of melee blows on a dummy: slashes through armour and poison.
- `economy.rs` (269): Tests that the posing economy gives the same rays and blade touches.
- `geometry.rs` (242): Tests of rays, colliders, a blast's reach, the size on screen and where the crowd stands.
- `wear_weapons.rs` (266): Tests of the dummy's worn things, its hung and held weapon, and its pieces.

### crates/trp-app/src/world/level

- `mod.rs` (54): The range as a file to edit in Blender and read back.
- `gltf_write.rs` (196): The range's layout written out as glTF.
- `level_read.rs` (270): The level's boxes, stands and starts, and a level read from glTF.
- `tests.rs` (95): Tests of the range written as a file for Blender and read back.

### crates/trp-app/src/world/loose

- `mod.rs` (59): Things on the ground, moved as the game's physics moves them.
- `flying.rs` (266): Loose things flown each frame and placed.
- `foreseen.rs` (105): What the inventory screen may put down next: the models' reader kept and one model asked ahead.
- `loose_state.rs` (386): How a thing goes loose, readied models, flights and the Loose resource.
- `models_read.rs` (377): Blasted things, blast effects, and the loose models read and prepared.
- `set_off.rs` (253): Setting loose things going.
- `tests.rs` (153): Tests of what uses the loose models and of the model asked ahead of a throw.
- `uses.rs` (54): What the loose models are used by, kept between frames, and the models nothing uses.

### crates/trp-app/src/world/range

- `mod.rs` (216): The test range as the simulation feels it, level ground and a few solid boxes.
- `as_world.rs` (215): The range as the simulation's World.
- `benches.rs` (371): The furniture, the benches, their tables and the workbench.
- `building.rs` (482): The range built: its hall, props and own boxes.
- `labels.rs` (216): The hall's lamps, the crates' labels, the barbed wire and what lies about.
- `starts.rs` (239): Starting places, and where the weapon tables and cabinets stand.
- `surfaces.rs` (300): Solids, surfaces, and the walls', windows' and crates' measures.
- `world_queries.rs` (292): Solids and placed things moved, and what the range answers about its world.

### crates/trp-app/src/world/range/tests

- `mod.rs` (25): Tests of the test range, with their shared helpers.
- `doors_containers.rs` (137): Tests of the doors worked and the containers' gestures.
- `layout.rs` (189): Tests of the range's layout, its level file and the benches' sight.
- `walking.rs` (187): Tests of walking the range: the open start, walls, boxes, the platform and the window.

### crates/trp-data/src

- `lib.rs` (23): Readers for your own Single Player Tushonka install: the game's files under it and SPT's database.
- `console.rs` (56): What the crates print, sent to the run's log as well as to the console.

### crates/trp-data/src/audio

- `mod.rs` (5): The game's sounds and its mixer presets.
- `audio_preset.rs` (498): The engine's audio group presets: what each group of the game's sound sources is made with.
- `thing_sound.rs` (291): The sounds of the things the hands work, the doors, containers and trunks of a map's scene.

### crates/trp-data/src/audio/sound

- `mod.rs` (39): The game's sounds as the install keeps them.
- `clips.rs` (351): Audio clips: FSB5 samples, Vorbis setups and decoding, IMA steps.
- `drops.rs` (151): The sounds of items dropped and of spent shells.
- `effect_clips.rs` (168): The bipod's, tinnitus, fracture and effects' sounds.
- `hands.rs` (232): The hands' sound players and the shot banks.
- `mixer.rs` (389): The master mixer, its groups and effects, aiming banks and headphones.
- `movement.rs` (319): The movement sounds: surfaces, gear, vaults, toggles and mounts.
- `tests.rs` (70): Tests of the sounds' reading: a Vorbis bank, a mark's sound and the headphones' settings.
- `ui_clips.rs` (122): The interface's and items' sounds.
- `voices.rs` (177): The voices' phrase banks and their reader.

### crates/trp-data/src/character

- `mod.rs` (13): The player's prefab: body, arms, ragdoll, root motion and what is worn.
- `arms.rs` (292): First-person arms: a skinned mesh that names its bones by path.
- `body.rs` (303): The player's body: its animator controller and the bones the first-person view depends on.
- `ragdoll.rs` (434): The ragdoll a dead character becomes, as the player prefab sets it up.
- `suits.rs` (148): The clothing suits of the install's customization list.
- `visor.rs` (203): The face shield's mask over the first person's view.
- `watch.rs` (93): The watch on the first-person hands' wrist.

### crates/trp-data/src/character/dress

- `mod.rs` (42): What a character wears over its clothing: armour, vests, backpacks, helmets, headsets.
- `fittings.rs` (133): Fittings of a skin or a part, and a part's clipping.
- `head_parts.rs` (201): Head models, worn parts, hinges and their figures, and fitting kinds.
- `hoods.rs` (122): Armbands and the hood with its levels.
- `loading.rs` (246): Surveying the dress bundles and loading a dress with its levels.

### crates/trp-data/src/character/inventory

- `mod.rs` (261): What a starting character carries, and what SPT's database says of an item as something to carry.
- `locale.rs` (128): The locale's words, items' short names and texts.
- `templates.rs` (177): Item templates' holders, slots, grids and cells.

### crates/trp-data/src/character/outfit

- `mod.rs` (156): An outfit that is not the install's: a body of S.T.A.L.K.E.R.'s, moved by the install's skeleton.
- `arm_fit.rs` (159): Laying meshes on the laid bones and fitting the arms.
- `arm_laying.rs` (314): Laying an arm's bones along the body's: bases, laid bones and hangs.
- `fitting.rs` (203): Fitting an outfit to the install's body.
- `hands.rs` (145): Skins, the hand's frame, the palm's box and laying both arms.
- `outfit_build.rs` (199): The rig's bones, an outfit's pieces and arms, and building an outfit.
- `rest_pose.rs` (173): Vector helpers, the rest place of bones and the moves that bend a limb.
- `tests.rs` (499): Tests of an outfit of the other game's: its bones, its build and how its rig follows the install's.

### crates/trp-data/src/character/player

- `mod.rs` (160): Tuning data on the player prefab: the springs and effectors of the first-person weapon animation.
- `body_colliders.rs` (268): The head's chain, the capsule, camera box, head colliders and limbs.
- `hands_data.rs` (200): The hands' shake, turn-away, sway, blind fire and force-react settings.
- `hands_load.rs` (351): Loading the hands' settings from the player's prefab, and the value readers.

### crates/trp-data/src/character/rootmotion

- `mod.rs` (155): The player's root motion table.
- `blend_weights.rs` (193): The weights of 1D, cartesian and directional blend trees.
- `node_read.rs` (143): Reading the table's nodes from its binary text asset.
- `table_queries.rs` (213): Loading the root motion table and asking it a state's velocity, length and weights.

### crates/trp-data/src/drawing

- `mod.rs` (8): What is drawn and how: particles, camera effects, glass and levels of detail.
- `camera_effects.rs` (290): What the camera's health effects are drawn with.
- `eye_burn.rs` (99): The eyes' burn after a flash, as the camera's prefab keeps it.
- `glass.rs` (88): The see-through glass of the face shields and the window magazines.
- `prism.rs` (310): The camera's PRISM effect as a camera prefab keeps it.

### crates/trp-data/src/drawing/lod

- `mod.rs` (142): A model's levels of detail: which of its renderers are drawn at what share of the screen's height.
- `nearest.rs` (189): A grid for the nearest triangle and the measured deviation of a shape.
- `shape_measure.rs` (277): TRP's measure of a level of detail: shapes, closing them and deviations.
- `shape_sources.rs` (102): The shapes of skins and trees, placed in and out through their nodes.
- `tests.rs` (223): Tests of the levels of detail: a renderer's levels and TRP's own measure of a coarser level.

### crates/trp-data/src/drawing/particles

- `mod.rs` (29): Unity's particle system as the install stores it.
- `curves.rs` (144): Particle curves and gradients: constant, curve, random between two.
- `decals.rs` (144): The deferred decals of grenades, bullets and blood.
- `effect_entries.rs` (338): The effects' pool entries, adapters, emitters, materials and shader queues.
- `effects.rs` (111): A particle effect read whole or by name.
- `muzzle.rs` (299): The muzzle's fume, heat, sparks, tracers' light and blasts.
- `system_data.rs` (319): A particle system's modules: shape, emission, sheet, forces, renderer, trails.
- `system_read.rs` (349): Reading a particle system, its renderer, material and built-in meshes.

### crates/trp-data/src/formats

- `mod.rs` (8): File formats decoded beyond Unity's bundles.
- `compress.rs` (320): Pictures compressed in blocks of four pixels by four, as the install keeps them.
- `curve.rs` (172): Unity animation curves: keyed values joined by cubic Hermite segments.
- `sketch.rs` (225): A plain picture of triangles seen straight on, with a grid, for measuring a model by eye.

### crates/trp-data/src/formats/fbx

- `mod.rs` (78): Reading a binary FBX file: the model of a weapon that is not the install's.
- `model_scene.rs` (353): An FBX scene's models and meshes, placed by their matrices.
- `motion.rs` (229): A binary FBX file's motion: its bones, and how each is placed and turned in its parent over the file's take.
- `records.rs` (200): Parsing a binary FBX file into records and their properties.

### crates/trp-data/src/formats/xray

- `mod.rs` (487): A model of the X-Ray engine's, the engine of the S.T.A.L.K.E.R.
- `tests.rs` (33): Tests of the X-Ray model's reading: its turns and a picture of one block.

### crates/trp-data/src/formats/xray_archive

- `mod.rs` (185): The X-Ray engine's packed archives, where the other game keeps the models and pictures not loose.
- `lzhuf.rs` (234): Unpacking LZHUF: the adaptive Huffman tree and the bit reader.
- `lzo1x.rs` (123): Unpacking LZO1X.
- `tests.rs` (104): Tests of the X-Ray archives' checksum and their LZO and LZHUF unpacking.

### crates/trp-data/src/interface

- `mod.rs` (7): The game's own screens, texts and item pictures.
- `edit_build.rs` (191): The edit build screen as the install's scene has it.
- `icon.rs` (449): An item's picture in the inventory, as the game makes it.

### crates/trp-data/src/interface/screen

- `mod.rs` (215): The game's own screens as its scenes hold them: the objects of one of the install's scene files.
- `battle.rs` (348): The battle views: mount icons, action panel, grid groups and sliders.
- `context_menus.rs` (334): Tooltips, context menus, small dialogs and the split dialog's look.
- `gear.rs` (117): The GEAR panel and its slots.
- `health.rs` (276): The HEALTH panel, its body parts, figures and slot views.
- `icons.rs` (325): The sprite names and icon tables of attributes, figures, effects and wishes.
- `item_views.rs` (303): The item and container views, scrolling, scroll bars, tints and sliders.
- `looks.rs` (163): Images' and texts' looks read from a scene's graphics.
- `materials.rs` (184): Resource sprites, build fonts and materials' shaders and properties.
- `notices.rs` (181): The version label, notices, named sprites and the malfunction glow.
- `overall.rs` (247): The OVERALL panel, its model camera and lights.
- `panel.rs` (282): A scene's panel: its nodes, row groups, elements and lookups.
- `panel_layout.rs` (227): Placing a panel's nodes: sizes, rows and what each node asks for.
- `pictures.rs` (285): A scene's pictures, sprite cuts, clips and cursors.
- `prefab_reader.rs` (291): Reading a prefab's objects, meshes, files and trees from a build.
- `quick.rs` (189): The quick slots and the ammo selector's look.
- `rigs.rs` (81): Rigs' layouts and their grids.
- `scene_read.rs` (360): Reading a scene file and walking its objects, transforms and behaviours.
- `sights.rs` (86): The sight panels and their details.
- `skills.rs` (319): The game's SKILLS page as the install's scenes hold it.
- `stash.rs` (168): The stash views, complex parts and filter tabs.
- `tabs.rs` (171): The tab bar and its parts.
- `tmp_font.rs` (223): TextMeshPro's fonts and glyphs, and a text's width.
- `windows.rs` (268): The repair, grid, info and tag windows and their rows.

### crates/trp-data/src/interface/tmp_text

- `mod.rs` (118): A text drawn as TextMeshPro draws it in the game.
- `face.rs` (177): A TextMeshPro face, its line widths and its material.
- `layout.rs` (267): Laying out, wrapping, sizing, drawing and shading a text.
- `tests.rs` (143): Tests of the TextMeshPro text: its size to fit, its layout, kerning and shading.

### crates/trp-data/src/interface/ui_trace

- `mod.rs` (33): The interface frame by frame, as the recorder writes it beside a trace and as the app writes it.
- `frame_state.rs` (287): The interface at a frame: its elements, canvases and draw order.
- `lines.rs` (186): A trace file's lines, their fields and the changes they carry.
- `side_by_side.rs` (125): Two interfaces paired element by element.
- `tests.rs` (224): Tests of the interface trace: reading and replaying its lines and pairing two traces' elements.
- `trace_file.rs` (199): The whole trace: frames, screens shown, events and what followed.

### crates/trp-data/src/level

- `mod.rs` (9): A level's light, sky, colliders, props and loot.
- `clouds.rs` (221): A level's clouds and its moon as the game sets them up.
- `level_colliders.rs` (457): A level's colliders as its scenes lay them.
- `level_light.rs` (396): A level's light as the game sets it up.
- `loot.rs` (107): What SPT's server fills a loot container with.
- `range_targets.rs` (69): The hideout shooting range's popper, read from the install to be put on the app's range.

### crates/trp-data/src/level/scene_props

- `mod.rs` (135): Things laid in a level's scene, to be drawn elsewhere: an object and all under it that is on.
- `prop_parts.rs` (177): A prop's renderers, box colliders, levels of detail and materials.
- `props_read.rs` (205): Reading a scene's props: their nodes, meshes and materials.

### crates/trp-data/src/level/tod

- `mod.rs` (54): A level's time of day as the game sets it up.
- `fields.rs` (202): A component's typed fields and its weighted curves.
- `level_settings.rs` (345): The clouds', wind, sky, weather and time-of-day settings of a level.

### crates/trp-data/src/setup

- `mod.rs` (7): Finding the install and its global settings.
- `database.rs` (243): SPT's copy of the server database: item templates and weapon presets.
- `engine.rs` (198): Engine-wide settings of the game build: the physics step and gravity.
- `hard_settings.rs` (381): The game's built-in settings object (the hard settings).
- `versions.rs` (350): The versions an install states for itself.

### crates/trp-data/src/setup/install

- `mod.rs` (136): Locating the install and its bundle manifest.
- `game_settings.rs` (314): The game's settings: player, bindings, axes, display, sound and the manifest.
- `profiles.rs` (108): The profile's skill levels, name and voices.
- `tests.rs` (70): Tests of locating the install: a file naming its folder, however it was saved or pasted.

### crates/trp-data/src/unity

- `mod.rs` (75): Unity's on-disk formats, as written by the Unity 2022.3 build Tarkov ships.
- `assets.rs` (265): A set of loaded bundles with cross-file reference resolution.
- `bc7.rs` (344): BC7 blocks decoded, and a BC7 picture made BC5 block by block.
- `bundle.rs` (199): UnityFS archive: a header, a compressed block table and a directory of files stored as one concatenated, block-compressed stream.
- `reader.rs` (82): Bounds-checked cursor over a byte slice with switchable endianness.
- `serialized.rs` (196): Serialized file: a table of typed objects plus the type trees needed to read them.
- `texture.rs` (465): Texture formats: where each mip level sits, and decoding of the block-compressed and crunched ones.
- `typetree.rs` (309): Type trees: the field layout a serialized file carries, and the reader that follows it.
- `value.rs` (245): Dynamically typed object data, as described by a type tree.

### crates/trp-data/src/unity/anim

- `mod.rs` (197): Animation clips.
- `clip_read.rs` (251): Reading a clip's curves, keeping, reducing and cutting them.
- `sampling.rs` (241): Sampling a clip's curves at a time, plain or smoothed.
- `tests.rs` (309): Tests of the animation clips: path hashes, rest rotations and the sampling of their curves.

### crates/trp-data/src/unity/extract

- `mod.rs` (78): Typed views of the objects this project needs: meshes, textures, materials and transform hierarchies.
- `colliders.rs` (121): Colliders, physic materials and rigidbodies on a hierarchy's nodes.
- `hierarchy.rs` (420): A prefab's hierarchy of nodes, renderers, levels of detail and grips.
- `meshes.rs` (271): Reading a mesh: its channels, vertex buffers and streamed data.
- `tests.rs` (73): Tests of the typed views: half floats, hierarchy paths and a part attached with its levels of detail.
- `textures.rs` (124): Reading textures' headers and data, and materials.

### crates/trp-data/src/weapons

- `mod.rs` (10): A weapon assembled from its parts, and what its parts carry.
- `grenade.rs` (62): A grenade's prefab beyond the hands: the parts let go of in a throw and the grenade thrown.
- `heat.rs` (133): The parts of a weapon that glow with its heat.
- `muzzle.rs` (474): A weapon's muzzle flash as its prefab keeps it.

### crates/trp-data/src/weapons/custom

- `mod.rs` (61): A weapon of one's own: a model not the install's, hung on the rig of one of the install's weapons.
- `assembly.rs` (312): Building a custom weapon's meshes and materials.
- `part_def.rs` (219): A custom part's definition, its kept nodes, marks and mesh picks.
- `part_models.rs` (262): Dressing and hanging parts, muzzles and magazines on a weapon.
- `picture_cache.rs` (272): A custom weapon's pictures, their compression and the kept cache.
- `pose_edits.rs` (148): The edits a custom weapon makes to the hands' pose.
- `registry.rs` (256): Finding custom weapons and parts in their folder and registering them.
- `tests.rs` (59): Tests of a weapon of one's own: its kept pictures and the hand's hold.
- `weapon_def.rs` (202): A custom weapon's definition: model, parts, bones, slots, hands and holds.

### crates/trp-data/src/weapons/sight

- `mod.rs` (238): The sights fitted to an assembled weapon.
- `lenses.rs` (171): A sight's ranges, zoom ring, reticle, lens nodes and lens shader.
- `sight_read.rs` (242): Reading a sight part from its prefab: slots, modes and figures.

### crates/trp-data/src/weapons/states

- `mod.rs` (141): The weapon animator's states: which clip each one plays and how fast, and the timed events the game's weapon logic waits for.
- `anim_state.rs` (179): Transitions, blend trees, layers and an animator state.
- `table_read.rs` (475): Reading the state table from an animator controller, and clips' lengths.

### crates/trp-data/src/weapons/tactical

- `mod.rs` (55): Tactical devices: the parts that carry a light or a laser, their modes and their figures.
- `device_parts.rs` (208): A device's light shafts, lights, lasers, flares and their materials.
- `device_read.rs` (336): Reading a tactical device, its volumes and textures from a prefab.

### crates/trp-data/src/weapons/weapon

- `mod.rs` (126): A weapon assembled as the game does it: a base prefab and one prefab for each part.
- `assembly.rs` (205): Opening a weapon's prefab and assembling it by name or by key.
- `attaching.rs` (145): Attaching parts and rounds to a weapon's tree.
- `folding.rs` (63): A folding stock's bones and the folded view.
- `moving_parts.rs` (216): Launchers, bipods, parts' clip binds, extraction and shell ports.
- `presets.rs` (402): Assembling a weapon from a preset of its parts.

### crates/trp-data/tests/install

- `main.rs` (36): Checks against a real install.
- `animation.rs` (202): Tests that read clips, the body controller, its blends and bindings.
- `clothing.rs` (373): Tests that read bots' looks, watches, suits, worn items, armbands and holsters.
- `drawing.rs` (434): Tests that read shadows, textures, camera effects and levels of detail.
- `head_gear.rs` (318): Tests that read head things' fitting, face shields, hinges, masks and glass.
- `impacts.rs` (177): Tests that read ballistic presets, impacts and a level's ballistic scripts.
- `locations.rs` (142): Tests that read the install's versions, blacklist and locations' floors and scenes.
- `outfits.rs` (426): Tests that fit another game's outfits and arms to the install's body.
- `particle_effects.rs` (400): Tests that read muzzle flashes, blasts, explosions, sparks, tracers and fumes.
- `screens.rs` (452): Tests that read the HEALTH, GEAR, battle bar, wish list and inspect panels.
- `sounds.rs` (247): Tests that read clips, shots, blasts, cases, voices and the mixer.
- `weapon_parts.rs` (191): Tests that read a weapon's assembly, pictures, cases, ports, marker and glass.

### crates/trp-physics/src

- `lib.rs` (393): The game's physics engine, PhysX 4.1, set up as the game's Unity sets it up from its settings.
- `corpse.rs` (166): A corpse as the game makes one, from the player prefab's ragdoll.
- `engine_setup.rs` (104): The shared PhysX engine, the cooked floor and the layers' filter.
- `joints.rs` (122): Character, spring and fixed joints between bodies.
- `loose.rs` (441): Things let fall and thrown: how the game sets one going, its rigid body and when it is taken away.
- `recorded.rs` (174): A recorded death set beside the engine's own, body by body and frame by frame.
- `setup.rs` (453): A recorded death's set-up beside the corpse as built.
- `stepping.rs` (122): Stepping a world, pushing bodies, reading poses, masses and touches.
- `sweeps.rs` (169): Sweeping spheres and boxes through a world, and kinematic bodies.
- `tests.rs` (195): Tests of the physics world: sweeps against meshes, boxes and spheres, a body taken out, a box at rest.
- `touches.rs` (112): Tagging bodies and collecting the touches PhysX reports.
- `world_bodies.rs` (278): Making a world and adding its static, floor, plane and moving bodies.

### crates/trp-physics/src/recorded_loose

- `mod.rs` (105): Things let fall and thrown, recorded, set beside the engine's.
- `falls.rs` (110): A recorded fall: its first impact, the ground and the drift.
- `setup_compare.rs` (347): The rounds in a thing and comparing the recorded start and setup.
- `trace_read.rs` (161): Reading a recorded throw and the model's key, colliders and hierarchy.

### crates/trp-physics/tests/physics

- `main.rs` (7): The tests of `trp-physics` that need the install or the recordings, in one binary.
- `corpse.rs` (70): Tests of the recorded deaths, each rebuilt and stepped as `bin/corpse` does.
- `loose.rs` (56): Tests of the install's things let fall as the game lets them fall, each to rest on the ground.
- `recorded_loose.rs` (106): Tests of the recorded things let fall and thrown, each set beside the rule that set it going.
- `shapes.rs` (46): Tests that every kind of shape is made and stepped, a convex hull resting as its box would.

### crates/trp-sim/src

- `lib.rs` (115): The character controller as a plain simulation.
- `install_settings.rs` (282): The movement, hands, launcher and empty-hands settings read from the install.
- `weapon_geometry.rs` (160): The weapon's root, length, line ends, rest clips and right-hand marker.
- `weapon_settings.rs` (232): The recoil and firearm settings read from a weapon and the install.

### crates/trp-sim/src/base

- `mod.rs` (12): Shared building blocks: Unity's math and random numbers, the animator, key bindings, settings and pacing.
- `hysteresis.rs` (155): A value carried between 0 and 1 by two curves, one for each way, as a face shield goes down and up.
- `marks.rs` (249): The marks in an animator's states that the game's logic waits for, by the game's own rule for when one fires.
- `math.rs` (292): The handful of Unity math functions the game's formulas are written in.
- `pacing.rs` (325): Runs the game's frame loop at a frame rate of its own, whatever rate the host draws at.
- `rig.rs` (357): The weapon's rig with no engine: its bones, each in its parent, as the weapon's animator has them.
- `systems.rs` (135): The controller's systems a host may switch off.

### crates/trp-sim/src/base/animator

- `mod.rs` (97): An animator: parameters, state machines, blend trees and the pose they make of a set of bones.
- `access.rs` (407): Making the animator, setting its parameters and reading its state and pose.
- `controller.rs` (425): The animator controller loaded from the install and its node and clip definitions.
- `defs.rs` (223): The controller's definitions: bone rests, conditions, transitions, states, layers and clips.
- `mixes.rs` (186): The animator's running state: playing states, blends, events, looks and mixes.
- `posing.rs` (448): Posing a controller's parts from its clips.
- `stepping.rs` (423): Starting and cutting states short, stepping each machine and evaluating the mix.
- `tests.rs` (149): Tests of the animator's transitions, conditions and layers.
- `transitions.rs` (308): The update, events, transition checks, lengths, weights and triggers.

### crates/trp-sim/src/base/bindings

- `mod.rs` (34): The game's keys as its input manager hears them, and its rules for changing them.
- `commands.rs` (429): Updating the preset: key states, commands and axes.
- `controls.rs` (181): The controls generated and chosen, and read from the install.
- `groups.rs` (238): Key groups and axis groups, their press choices and priorities.
- `keys.rs` (136): Key codes and their names, press types and key presses.
- `preset.rs` (258): The preset's state machine types and making a preset.
- `tests.rs` (323): Tests of the game's key bindings: presses, holds, combinations and reserved keys.

### crates/trp-sim/src/base/settings

- `mod.rs` (208): Everything the movement simulation reads from the game's data, gathered into one value.
- `reading.rs` (165): Reading the movement settings from the install.
- `weights.rs` (162): Weight limits and reweighing the settings by strength and load.

### crates/trp-sim/src/base/unity_random

- `mod.rs` (423): Unity's scripting random numbers as the engine makes them, and the game's tables filled from them.
- `tests.rs` (98): Tests of Unity's random numbers and the game's tables filled from them.

### crates/trp-sim/src/characters

- `mod.rs` (7): Characters seen from outside: bots, the puppet, the inventory's figure, levels of detail and the range's targets.
- `bots.rs` (67): What SPT's server makes of a bot's look as it generates one, as far as the range's dummies need it.
- `popper.rs` (379): A popper of the hideout's shooting range, a steel plate on a hinge that a hit folds back.

### crates/trp-sim/src/characters/figure

- `mod.rs` (242): The character as the game's own screens show it, as the inventory's OVERALL figure.
- `figure_data.rs` (283): The figure's and its weapon's data loaded from the install.
- `grip.rs` (174): Holding the weapon in the figure's hands.
- `operations.rs` (109): Random idles, weapon operations, patrols and adding ammunition.
- `skeleton.rs` (151): Small vector helpers, the blender and the named skeleton.
- `stepping.rs` (197): Making a figure, stepping it and reading its pose and bones.

### crates/trp-sim/src/characters/lod

- `mod.rs` (350): Which level of detail a camera draws of a group, by Unity's rule and by TRP's own.
- `tests.rs` (302): Tests of the levels of detail: the engine's pick, TRP's own rule and the props' levels.

### crates/trp-sim/src/characters/puppet

- `mod.rs` (149): A character no one is at the keys of, seen from outside with a firearm in its hands.
- `access.rs` (304): Making a puppet, stepping it and reading its poses, recipes and bound.
- `drawing.rs` (125): The recipe a frame is made from, what is kept and the drawing between frames.
- `frame.rs` (261): Making one frame of the puppet and the rig bones it follows.
- `pose.rs` (169): The puppet's pose, how it is posed and the bound around its colliders.

### crates/trp-sim/src/combat

- `mod.rs` (8): A shot after it leaves the weapon, and a grenade's blast.
- `blast.rs` (367): What a grenade's blast does to a character, the player's or any other's.
- `fragments.rs` (178): A grenade's fragments, shots of its fragment round flown from the blast.
- `impacts.rs` (403): What a bullet's hit plays: which hits play, where, how loud, and whether seen.
- `shells.rs` (348): A spent case's flight, in jumps cast against the world until the casts are spent.
- `tracers.rs` (396): A tracer round's trace: which bullets show one, and the streak each is drawn as.

### crates/trp-sim/src/combat/ballistics

- `mod.rs` (84): A shot after it leaves the weapon: its flight, what it meets, and what each hit does to it.
- `bullets.rs` (226): Ray hits, body references, the bullet world, bullets and hits.
- `flight.rs` (306): Shooting bullets, ticking their flight, handling hits and the linecasts.
- `fragmentation.rs` (169): Fragments thrown from a bullet's hit.
- `material.rs` (70): The numbers and names of the materials a bullet meets.
- `rounds.rs` (172): Rounds, fuses and the surfaces they hit.
- `tests.rs` (188): Tests of a shot's spread, flight and penetration.
- `trajectory.rs` (119): The G1 drag table and a round's precomputed trajectory.
- `weapon_figures.rs` (180): A weapon's figures for spread and deviation, and the iron shot direction.

### crates/trp-sim/src/condition

- `mod.rs` (8): The character's health, armour, skills and experience.
- `experience.rs` (201): The character's experience and level, and what the game counts as experience in a raid.
- `mastering.rs` (292): Weapon mastering: the weapons' families, their points and the level the game makes of them.

### crates/trp-sim/src/condition/armor

- `mod.rs` (32): The armour a character wears and what it does to a hit.
- `damage.rs` (135): Applying shots and blows through armor to the body.
- `pieces.rs` (270): Armor pieces, their order, vest skills and the speed and ergonomics penalties.
- `rules.rs` (188): Collider types, plates, armor rules, resistance and armor slots.
- `tests.rs` (299): Tests of the armour's pieces, face shields, wear and damage reduction.
- `wearer.rs` (288): Shots and blasts taken by the wearer of armor, and the mark limits.

### crates/trp-sim/src/condition/earning

- `mod.rs` (402): How the character's skills and weapon mastering are earned.
- `tests.rs` (163): Tests of how skills and mastering are earned: the awards, fatigue and first levels.

### crates/trp-sim/src/condition/health

- `mod.rs` (134): The character's health: body parts, effects, damage, medicine and death.
- `conditions.rs` (275): Part states, the conditions the body is in, events, refusals and health skills.
- `damage.rs` (349): Damage and its effects, falls, exertion and the barbed wire tally.
- `effect.rs` (246): The kinds of effect, their stages, medical work and the effect itself.
- `lifecycle.rs` (381): Adding effects and running them through their stages.
- `med_work.rs` (246): Medicine at work: starting, updating, residue and restoring.
- `medicine.rs` (198): Cures, medical classes and the medicines read from the install.
- `parts.rs` (160): Body parts, damage kinds and the odds of a chance.
- `readings.rs` (230): Making the health, taking up a saved state and reading it.
- `settings.rs` (211): The health settings read from the install.
- `tests.rs` (402): Tests of the character's health: hunger, falls, damage, bleeding and effects at work.
- `treatment.rs` (235): Choosing the parts to help and using medical items.
- `update.rs` (281): The step: regular updates, health, energy, hydration and death.

### crates/trp-sim/src/condition/health/stimulator

- `mod.rs` (236): Stimulators' buffs, and the buffs food and drink carry.
- `readings.rs` (114): What running stimulators give: listings, boosts, buffs and side effects.
- `running.rs` (291): Adding a stimulator and running its buffs through their stages.

### crates/trp-sim/src/condition/skills

- `mod.rs` (52): The character's skills: each skill's progress and what its level makes of the buffs.
- `buff_table.rs` (162): Every skill's buffs by level.
- `buffs.rs` (271): The buffs worked out from the skill levels and their definitions.
- `catalogue.rs` (243): The skills, their names, classes, weapons and level curves.
- `levels.rs` (129): The skill levels held and the covert movement they give.
- `tests.rs` (89): Tests of the skills' levels, buffs and names.

### crates/trp-sim/src/effects

- `mod.rs` (10): What is drawn around the action, as rules with no engine: particles, decals and screen effects.
- `blood.rs` (409): Blood on the screen: where the drops go for a hit and a bleeding, and when they show.
- `decals.rs` (128): The game's deferred decals: the box a decal is laid in and what its shader does inside it.
- `emitters.rs` (246): The game's older particle emitters and the pool's light.
- `flare.rs` (313): The tactical lights' lens flares, and the eyes' protection that dims them.
- `sparks.rs` (163): The game's sparks: a ring of slots, each spark drawn as a streak the shader moves.

### crates/trp-sim/src/effects/particles

- `mod.rs` (115): Unity's particle system run: particles born, stepped by the system's modules and drawn.
- `collision.rs` (240): Particles meeting the world and the ray budgets for it.
- `cubes.rs` (190): A particle system's meeting with the world at medium and low quality.
- `emission.rs` (246): Playing a system and emitting particles from it.
- `particle.rs` (157): Emit parameters, particles, drawn output, the world and placement.
- `shapes.rs` (145): Emission shapes, texture sheets and the small vector maths.
- `stepping.rs` (330): Stepping particles, their trails, lights and what is drawn.

### crates/trp-sim/src/effects/particles/tests

- `mod.rs` (72): Tests of the particle system, with their shared helpers.
- `collision.rs` (165): Tests of particles meeting the ground and the rays shared among systems.
- `lifetime.rs` (195): Tests of velocity over life, trails, sub-emitters, varying values, puffs, births and drag.
- `playing.rs` (238): Tests of playing a system: bursts, shapes, pauses, long frames, sheets and local space.

### crates/trp-sim/src/effects/screen_effects

- `mod.rs` (165): What health's effects do to the screen, frame by frame.
- `desaturation.rs` (103): The health desaturation of the screen.
- `eye_burn.rs` (123): Eyes burnt by a bright light and the spots they leave.
- `frame.rs` (285): Setting the figures and working out each frame's screen effects.
- `hits.rs` (167): Effect sources, hit reactions and the listing of effects on screen.
- `tests.rs` (281): Tests of health's screen effects: burns, blurs, vignettes and the contusion.

### crates/trp-sim/src/effects/visor

- `mod.rs` (360): The face shield's mask over the first-person view.
- `tests.rs` (222): Tests of the face shield's mask: its cracks, fade, swing and the shield it goes by.

### crates/trp-sim/src/held

- `mod.rs` (10): What the hands hold besides a firearm, and what they work in the world.
- `empty_hands.rs` (154): Hands that hold nothing.
- `grenade_hands.rs` (399): The hands that hold a grenade.
- `interactions.rs` (493): The hands' gestures on the world: opening and shutting a container, pushing and pulling a door, taking up a thing.
- `rangefinder.rs` (243): The handheld rangefinder in the hands: its controller and its readout.
- `watch.rs` (101): The watch on the first-person hands' wrist: the time it shows and its three hands' turns.

### crates/trp-sim/src/held/knife_hands

- `mod.rs` (144): The hands that hold a knife or another melee weapon.
- `blade.rs` (97): Blow damage, the swing and the blade's sweep.
- `tests.rs` (53): Tests of the knife's blade, its damage and the direction of its hits.
- `working.rs` (286): Making the knife hands, drawing, striking, hiding and stepping them.

### crates/trp-sim/src/held/meds_hands

- `mod.rs` (458): The hands that use an item: medicine, food or drink.
- `tests.rs` (177): Tests of an item's use in the hands: its parts, cancels and the way back.

### crates/trp-sim/src/held/things

- `mod.rs` (197): The things of the world the hands work: a door, a container, a car's trunk.
- `actions.rs` (167): Thing sounds, kinds, states, actions and the menu a thing offers.
- `breaching.rs` (167): Breaching, breaking in, kicking open, unlocking and the gestures at a door.
- `reading.rs` (141): Making doors, containers and trunks, and reading their state and menu.
- `swinging.rs` (249): Working a thing, its state changes and the swing stepped on the clock.
- `tests.rs` (291): Tests of the doors and containers the hands work: their waits, swings and sounds.

### crates/trp-sim/src/items

- `mod.rs` (11): What is carried: the inventory, examining, searching, repairing and the edit build screen's rules.
- `examine.rs` (174): Examining a thing, and which kinds of thing the character knows.
- `magazine_process.rs` (188): A magazine loaded or unloaded a round at a time.
- `magazine_view.rs` (157): A magazine's rounds as drawn on its model.

### crates/trp-sim/src/items/edit_build

- `mod.rs` (20): The edit build screen's rules that are not the engine's.
- `placement.rs` (263): Placing the slots' labels around an ellipse and spacing them out.
- `slots.rs` (248): Mod classes, build slots, which parts fit and assembling a build.
- `tests.rs` (50): Tests of the edit build screen: a circle's lengths, the spacing, the slots on the ellipse and their classes.

### crates/trp-sim/src/items/inventory

- `mod.rs` (87): What the character carries and where each thing sits, by the game's rules.
- `chambers.rs` (144): Chambers, cylinder camoras and which rounds go together.
- `equipment.rs` (305): The equipment, the carried weight and items made from lists.
- `item.rs` (409): The item, its tags, lights and repairs, and room in its grids and slots.
- `kinds.rs` (353): Item kinds, the places items go and the refusals of a move.
- `loading.rs` (319): Loading and unloading rounds, weapon builds, chambering and unfit rounds.
- `magazines.rs` (168): Choosing, unloading and swapping magazines.
- `menus.rs` (174): The reload and install menus and applying an item.
- `moves.rs` (364): Finding items, and moving, taking out and using them up.
- `rounds.rs` (260): Rounds within reach, magazine rounds and the words for ammunition.
- `rules.rs` (217): Raid limits and conflicts between equipped items.
- `switches.rs` (169): Switches, face shields, blindness protection and folding parts.
- `transfers.rs` (339): Splitting, ordering, quick finding, picking up, equipping and binding items.
- `wearing.rs` (104): The loot order, the slots to wear a kind in and fitting bits.

### crates/trp-sim/src/items/inventory/tests

- `mod.rs` (31): Tests of the inventory, with their shared helpers.
- `grids.rs` (202): Tests of item grids, the room found, an item's size from its parts and folding.
- `moves.rs` (324): Tests of equipment slots, moving, stacking, splitting, putting on and binding things.
- `reloads.rs` (334): Tests of cylinders, tubes, reloads and magazines loaded and swapped.
- `rules.rs` (311): Tests of room for things sent over, conflicts, locked slots, raid limits and weight.
- `weapons.rs` (173): Tests of weapons coming apart, magazines' rounds and chambers.

### crates/trp-sim/src/items/item_attributes

- `mod.rs` (303): A thing's figures as the inspect window lists them.
- `firearms.rs` (149): The attribute lines of weapons and ammunition.
- `meds.rs` (168): The attribute lines of medicine and stimulators, and the buffs they give.
- `protection.rs` (182): The attribute lines of armour, penetration and plates.

### crates/trp-sim/src/items/item_menu

- `mod.rs` (410): The menu of a right click on a thing in the inventory, in a raid.
- `tests.rs` (162): Tests of the right click's menu: which lines a thing shows and whether each can be done.

### crates/trp-sim/src/items/repair

- `mod.rs` (31): Repairing a thing with a repair kit: what the repair window shows and what a repair does.
- `kit_buffs.rs` (247): The buffs a repair kit may roll and their chances.
- `planning.rs` (322): Planning a repair: durability, degradation, price and the outcome.
- `settings.rs` (161): The repair settings, the parts and the repairers.
- `tests.rs` (151): Tests of repairs: the window's reckoning, a kit's mending and the buffs' draws.

### crates/trp-sim/src/items/search

- `mod.rs` (141): Searching a container, and what the character has not yet searched or found.
- `filling.rs` (135): Fill rules and drawing loot into a container.
- `searcher.rs` (323): The searcher: what is hidden, beginning and stopping a search, and its advance.

### crates/trp-sim/src/lighting

- `mod.rs` (8): A level's light as the game works it out.
- `ambient.rs` (201): The game's ambient light as spherical harmonics.
- `clouds.rs` (306): Customs' clouds by the game's rules.
- `hbao.rs` (328): The game's ambient occlusion, its graphics setting `SSAO`.
- `prism.rs` (181): The camera's PRISM effect: the exposure, the vignette and the tonemap.
- `shading.rs` (143): The game's surface light, as its deferred passes work it out.

### crates/trp-sim/src/lighting/tod

- `mod.rs` (80): A level's time of day as the game runs it.
- `atmosphere.rs` (190): Scattering colours, atmosphere samples, the controller update and the harmonics.
- `celestials.rs` (209): The sun's and moon's places and the sky's state at a moment.
- `globals.rs` (105): The sky's shader globals and the atmosphere stored for a recorded moment.
- `sky.rs` (201): The sky read from a level: periods, the controller, the atmosphere and scattering.

### crates/trp-sim/src/locomotion

- `mod.rs` (8): How the character moves: walking, collision, vaulting, stamina, the body's animator and the free look.
- `head_look.rs` (259): The head's free look.

### crates/trp-sim/src/locomotion/body

- `mod.rs` (247): The player's body: its animator, run for what the movement and the first-person view take from it.
- `arms.rs` (224): The body's bones, the arm pose and how the arms reach for it.
- `body_data.rs` (402): The body's animator data loaded from the install, and its ids.
- `drawn.rs` (289): The drawn bones, the ribcage, the weapon and the arm poses.
- `inputs.rs` (293): Making the body, taking its events and setting what drives it.
- `update.rs` (326): The body's update and what it reports: motion, turns, states and progress.

### crates/trp-sim/src/locomotion/collision

- `mod.rs` (50): The character's own collision, for a world that is one line of ground with steps in it.
- `block.rs` (245): Boxes: overlaps and pushes against spheres, capsules and upright boxes.
- `course.rs` (150): Treads and the course of ground a capsule settles on.
- `tests.rs` (193): Tests of the character's own collision: walls, boxes, steps, edges and other characters.
- `upright.rs` (203): Upright capsules entering blocks, the sites settled on and the reach.

### crates/trp-sim/src/locomotion/movement

- `mod.rs` (474): Movement, as specified in `docs/spec/movement.md`.
- `blind_fire.rs` (107): Blind fire over and around cover.
- `commands.rs` (397): Reading the commands each step: hands, tilt and what was heard.
- `creation.rs` (428): Building the movement, its weight, skills, conditions and hits.
- `footsteps.rs` (159): What is carried, footsteps, hearing and the body's input.
- `input.rs` (293): The movement states and the input frame.
- `interaction.rs` (399): Interactions: working, searching, unlocking, kicking doors and picking up.
- `jumps.rs` (380): Jumps, vaults and the obstacle checks.
- `left_stance.rs` (191): The left shoulder in the movement.
- `motion.rs` (174): Setting the state, applying motion, falling and travelling through the world.
- `orders.rs` (213): The step, sprint, prone and pose commands, and approaching a spot.
- `pom.rs` (165): The head's push out of walls.
- `posture.rs` (316): Pose changes, falls, landing limits, slope, tilt smoothing and body turn.
- `prone.rs` (200): Idle, sidestep, roll and prone updates, and aligning to the surface.
- `reports.rs` (160): What a step reports: interactions, moves, actions, kicks and approaches.
- `room.rs` (314): Room to move, the checks of the world before the body steps aside, rolls, lies down or stands.
- `slope.rs` (209): The ground's slope under the character.
- `states.rs` (325): Entering each movement state and turning toward the look.
- `stepping.rs` (295): The step on the clock, search waits, work stamina and body height lag.
- `tuning.rs` (217): The movement's constants and the move, jump, step and vault data read from the install.
- `views.rs` (388): What the movement shows the rest: capsule, poses, camera frames, overlaps.
- `walking.rs` (409): Walking and sprinting: speed selection, move direction, sprint and landing.
- `world.rs` (328): The capsule, the world trait, the stopwatch and flat ground.

### crates/trp-sim/src/locomotion/movement/mount

- `mod.rs` (110): The mount's own state.
- `entering.rs` (202): Asking for a mount, answering its search, entering and leaving it.
- `holding.rs` (245): Staying mounted: the update, the approach, the body, looking and rotating, and drain.
- `reading.rs` (106): Reading the mount: its view, figures, rig origin, events, tilt and leaving it.

### crates/trp-sim/src/locomotion/stamina

- `mod.rs` (132): What the character pays for moving and for holding a weapon up.
- `arms.rs` (190): Melee, wounds, weapon switches, holding breath and aiming.
- `feeds.rs` (150): Making the stamina and feeding it buffs, health, weight and stance.
- `moves.rs` (209): The costs of vaulting, climbing, sprinting, jumping, standing up and moving.
- `pool.rs` (162): The drains and the pools of legs, arms and oxygen.
- `settings.rs` (321): The stamina graphs and settings read from the install, with their skill factors.
- `tests.rs` (365): Tests of the stamina's pools: what each move and aim costs, and the refill.

### crates/trp-sim/src/locomotion/vaulting

- `mod.rs` (466): Vaulting and climbing: measuring what stands ahead and choosing the move.
- `tests.rs` (181): Tests of vaulting and climbing: what is vaulted, what is climbed and what refuses a move.

### crates/trp-sim/src/recordings

- `mod.rs` (7): Recordings of the game set beside the simulation: what the comparison tools share.
- `fall_trace.rs` (293): A recorded corpse's fall set beside the rule's, for the tools that compare it.
- `particle_trace.rs` (349): A recording's particle systems as the recorder writes them in a world file.
- `script.rs` (234): A scripted input sequence, as the recording plugin plays it.

### crates/trp-sim/src/recordings/health_replay

- `mod.rs` (65): A recording's health set beside the simulation's, for the health replay and its test.
- `in_hand.rs` (115): The meds hands' rig, variants and loops taken in hand for the replay.
- `log_reading.rs` (173): Reading a health recording's log of frames and hands in use.
- `replay.rs` (352): Running a health recording through the simulation.

### crates/trp-sim/src/recordings/voice_trace

- `mod.rs` (50): A recording's bots' voices set beside the speaker's rules, for `bin/botsounds --voices`.
- `comparing.rs` (234): Comparing what was said with the recording, trace by trace.
- `reading.rs` (157): Reading the recorded world and the voice trace.
- `replaying.rs` (223): Running a voice trace through the speakers and what they say.

### crates/trp-sim/src/sound

- `mod.rs` (6): What is heard: footsteps, a weapon landing, the voice, hearing under headgear.
- `landing.rs` (235): A weapon heard as it lands.

### crates/trp-sim/src/sound/hearing

- `mod.rs` (32): A helmet's cut of what is heard.
- `headphones.rs` (218): Sound groups, decibel sums and the headphones' fading level.
- `params.rs` (150): The hearing parameters and choosing a template by what is worn.
- `templates.rs` (171): Deafness and the hearing templates with their presets.
- `tests.rs` (235): Tests of the cut under headgear: the templates, their choice by what is worn and the recorded fade.

### crates/trp-sim/src/sound/speaker

- `mod.rs` (151): The character's own voice.
- `phrases.rs` (159): Banks of phrases, statuses, situations and what was said.
- `reactions.rs` (207): The update and what a speaker reacts to: damage, effects, death, breath and shields.
- `saying.rs` (241): Making a speaker, steering it, its awareness and saying and playing phrases.
- `tests.rs` (308): Tests of the voice: grunts, cries, breathing, a face shield's muffle and the choice of clips.

### crates/trp-sim/src/sound/steps

- `mod.rs` (163): The sounds of the player's own moving: footsteps, sprints, jumps, landings, crawls and turns.
- `banks.rs` (133): Step sound kinds, the banks they come from and a sound to play.
- `choosing.rs` (349): Making the step sounds, covert walking, kit noise and choosing a sound from a bank.
- `settings.rs` (163): The step settings, surface checks and the kit's figures.
- `tests.rs` (234): Tests of the sounds of moving: footsteps and their lists, the kit, landings and turns.
- `update.rs` (192): The update: state changes, landings, idling and turns.

### crates/trp-sim/src/weapons

- `mod.rs` (17): The firearm in the hands: its logic, motion, recoil, sights, wear and stances.
- `arms.rs` (369): The arms' hold on the weapon: the limb solver as the game runs it, with no engine types.
- `blind_fire.rs` (41): Blind fire's hand-over between the parts.
- `breath.rs` (384): The weapon's breathing.
- `hand_shake.rs` (213): The hand shake after a quick draw.
- `left_stance.rs` (263): The left shoulder, the weapon carried on the left side of the body.
- `quick_draw.rs` (79): The quick draw of the holster's weapon.

### crates/trp-sim/src/weapons/firearm

- `mod.rs` (250): The weapon's own logic: trigger, fire modes, rounds, reloading, being drawn and put away.
- `aiming.rs` (160): Aiming, lowering the aim and telling the body of it.
- `animation_marks.rs` (276): What each animation mark does to the firearm.
- `bipod.rs` (123): Folding the stock, mounting and the bipod.
- `chambering.rs` (251): Loading, discharging, unloading and rechambering by hand.
- `checks.rs` (294): Fire mode changes, chamber and magazine checks, examining and the backpack.
- `drawing.rs` (327): Skill speeds, mastering, and drawing and hiding the weapon.
- `firing.rs` (238): Firing single shots and automatic bursts.
- `inventory.rs` (223): Moving magazines from the inventory and the chamber checks before loading.
- `making.rs` (218): Creating the firearm and choosing its magazine's variants.
- `operation.rs` (249): The operations a firearm runs, the blends and the chambering choices.
- `reload.rs` (183): Reloading a magazine and the bolt catch.
- `settings.rs` (333): Fire modes, the firearm's settings and the events it reports.
- `shells.rs` (246): Reloading shells one at a time.
- `starting.rs` (200): Blind fire and starting the firearm drawn, folded or taken over.
- `stepping.rs` (248): The step on the clock, left stance, idling and appearing.
- `trigger.rs` (203): The animator's values, the idle queries and the trigger.

### crates/trp-sim/src/weapons/firearm/chambers

- `mod.rs` (52): Weapons whose rounds lie in chambers of their own and not in a magazine under a bolt.
- `barrels.rs` (229): Barrels loaded one by one, the rocket's trigger and their marks.
- `cylinder.rs` (339): A revolver's cylinder: turning, the trigger, reloading and its marks.
- `own_chamber.rs` (296): Loading and unloading a chamber of its own from the inventory.
- `shown.rs` (250): Counting, filling and showing the rounds and cases in a firearm's chambers.

### crates/trp-sim/src/weapons/firearm/launcher

- `mod.rs` (183): An underbarrel grenade launcher in the hands.
- `fitting.rs` (145): Fitting an underbarrel launcher, reading its state and toggling to it.
- `operating.rs` (248): Switching to the launcher, its trigger, and reloading, loading and discharging it.
- `update.rs` (272): The launcher's range, checks, aim, update and animation marks.

### crates/trp-sim/src/weapons/firearm/malfunctions

- `mod.rs` (117): The weapon's malfunctions, wear and heat in the hands.
- `clearing.rs` (162): Examining, checking and clearing a malfunction, and the repair marks.
- `rolling.rs` (144): Rolling a malfunction on a shot and switching the firearm into it.
- `wear_inputs.rs` (152): Setting the wear inputs and heat, and reading the malfunction state.

### crates/trp-sim/src/weapons/firearm/tests

- `mod.rs` (249): Tests of the firearm in the hands, with their shared helpers.
- `drawing.rs` (190): Tests of the hands going down for the inventory and drawing and putting away.
- `firing.rs` (197): Tests of fire modes, rates, running dry, magazine checks and the aim.
- `reloading.rs` (217): Tests of reloads: mounts, chambered rounds, sprints, the quick reload and the old magazine.

### crates/trp-sim/src/weapons/hands

- `mod.rs` (359): Procedural weapon motion.
- `aiming.rs` (266): Pose blends, breath, shots and aiming.
- `bob.rs` (244): Tracking springs, the walk bob and the stiffened weapon bone.
- `camera.rs` (323): The frame on the clock, the aim sway and the camera's position and rotation.
- `creation.rs` (205): Making the hands and carrying them on from other hands.
- `feeds.rs` (404): Forces, shakes and what feeds the hands: rig, head, health, load and handling.
- `fixed_step.rs` (371): The fixed step, turning away from walls, overlaps and the sight line.
- `input.rs` (189): What the hands carry and take in, the shakes and the turn-away state.
- `rests.rs` (200): Rests, mounting, blind fire and measuring the overlap.
- `settings.rs` (323): The hands' settings, weapon handling, the sight line and the aim point.
- `springs.rs` (165): The blender, the spring and the sway the hands are moved by.
- `tests.rs` (99): Tests of the weapon's motion: the springs, the sway, the recoil's forces and a blast's shake.
- `transforms.rs` (322): Where the weapon is put: plain, shifted and carried.
- `zoom.rs` (260): Hip inaccuracy, sights, zoom, skills and the field of view.

### crates/trp-sim/src/weapons/malfunction

- `mod.rs` (73): A weapon's wear, its malfunctions and its heat, as pure rules.
- `rolling.rs` (285): Heat, the chances and rolling a malfunction on each shot.
- `settings.rs` (305): The malfunction settings and the weapon's and the round's figures.
- `tests.rs` (178): Tests of a weapon's wear, malfunction chances and heat, against the recorded AKM.

### crates/trp-sim/src/weapons/mounting

- `mod.rs` (121): Mounting and the bipod.
- `mount_data.rs` (150): What a mount asks for and the data worked out for it.
- `search.rs` (249): Vector helpers and the search for a place to mount on, standing or prone.
- `settings.rs` (173): The mount's settings read from the install.
- `telling.rs` (138): The events and motion a mount tells its host.
- `tests.rs` (74): Tests of the search for a mount, its approach and a forward mount's data.
- `weapon_figures.rs` (136): A weapon's bipod and mount figures.

### crates/trp-sim/src/weapons/muzzle

- `mod.rs` (31): A weapon's muzzle flash.
- `flash.rs` (204): The muzzle jet mesh, the flash and its light.
- `haze.rs` (234): Smoke draws, fume puffs, sparks and the barrel haze.
- `smoke.rs` (178): The smoke left by shots and its corners.
- `tests.rs` (403): Tests of the muzzle: its jets, light, smoke, fumes, sparks, haze and trail.

### crates/trp-sim/src/weapons/recoil

- `mod.rs` (251): Recoil, as specified in `docs/spec/weapon-handling.md`.
- `rotation.rs` (227): The rotation process: pushes, steps and the return trajectory.
- `settings.rs` (146): Shot groups, curve recoil, the recoil settings and a recoil process.
- `tests.rs` (213): Tests of recoil: one shot, automatic fire, the stances and the weapon's curves.

### crates/trp-sim/src/weapons/sights

- `mod.rs` (236): The sights a weapon aims by, as the game picks and works them.
- `kept.rs` (57): What the sight items keep, read off the sights and given back to them.
- `making.rs` (302): Making the sights from a weapon and reading which sight is in use.
- `optics.rs` (161): Scope rotation, optics, zoom values and aiming sensitivity.
- `switching.rs` (217): Changing scope, cycling scopes and magnification, the zoom frame and the eye.
- `turning.rs` (251): Calibrating and turning optics and collimators by distance.
- `zeroing.rs` (246): Folding sights, zeroing, ballistics, the iron line and the launcher line.
- `zoom_handler.rs` (164): The zoom handler and the zoom input that drives it.

### crates/trp-sim/src/weapons/tactical

- `mod.rs` (23): Tactical devices: the lights and lasers on a weapon, and the head light.
- `headwear.rs` (108): Head-mounted lights and the requests to switch them.
- `laser.rs` (88): Laser figures, the laser's light and the head pose.
- `switching.rs` (211): The keys, the events and the devices' frame.
- `tests.rs` (165): Tests of tactical devices: the keys, the modes, the hip penalty, the lights and the head light's switch.
- `weapon_devices.rs` (148): The weapon's tactical devices, their modes and the requests to switch them.

### crates/trp-sim/src/weapons/weapon_animator

- `mod.rs` (175): A weapon's own animator, run as the engine runs it, with the marks the game's logic waits for.
- `animator_data.rs` (236): The weapon animator's data and parts loaded from the install.
- `parameters.rs` (173): Setting and reading the weapon animator's parameters, states and poses.

### crates/trp-sim/tests

- `examine.rs` (219): Tests of examining things: what is known, how long an examine takes and what is refused.
- `search.rs` (411): Tests of searching a container, its pace and its order.

### crates/trp-sim/tests/sim

- `main.rs` (60): The tests of trp-sim that need the install or the recordings, gathered in one binary.
- `aim_sway.rs` (51): Tests of a heavy weapon's sway as its sights come up.
- `ambient.rs` (116): Tests of Factory by day's light as the install sets it up, beside a recorded scene of it.
- `armor.rs` (200): Tests of the armour read from the install and dressed as the game gathers it, and of the hit colliders.
- `attributes.rs` (257): Tests of a thing's figures as the inspect window lists them, from the install.
- `blind_fire.rs` (245): Tests of blind fire with the install's body animator, the stock AKM and its motion.
- `build_swap.rs` (110): Tests of a reload that goes on through the weapon's build made anew.
- `builds.rs` (251): Tests of a weapon made from its own parts, from the install.
- `clouds.rs` (122): Tests of Customs' clouds and moon as the install sets them up, beside a recorded scene.
- `earning.rs` (235): Tests of what the hands and the health give the skills, as the install's weapons and items run.
- `every_weapon.rs` (251): Tests that put every weapon of the install through what the app does with it.
- `fitting.rs` (70): Tests of the head's things fitted to one another, by the install's items.
- `fixture.rs` (28): The install, read once for the whole binary.
- `free_look.rs` (235): Tests of the head's free look, measured with the real install's data and the stock AKM.
- `hbao.rs` (150): Tests of the game's ambient occlusion beside a recorded scene of Customs.
- `head.rs` (93): Tests of the head's push out of walls, with the install's prefab, movement and body animator.
- `health.rs` (74): Tests of the health read from the install, its figures and every thing to use on it.
- `health_replay.rs` (191): Tests that replay the recorded medicine courses and set the health beside the game's.
- `hearing.rs` (73): Tests of a helmet's cut of what is heard, by the install's items.
- `impacts.rs` (57): Tests of what a bullet's hit shows, beside the recorded ballistics courses.
- `incompatible_ammo.rs` (130): Tests of rounds a magazine takes but the chamber does not.
- `launcher.rs` (374): Tests of the underbarrel launcher in the hands, on the install's own controllers.
- `left_stance.rs` (122): Tests of the left shoulder with the install's body animator.
- `limits.rs` (105): Tests of the speed limits from armour and from being hit, with the install's movement and database.
- `lod.rs` (135): Tests of the game's pick of the levels of detail against three recordings of it.
- `look.rs` (172): Tests of a weapon's animations drawn on curves without corners, and of the result left unchanged.
- `magazine_clips.rs` (67): Tests of a magazine's own clips, by the fitted magazine and through a reload.
- `malfunctions.rs` (373): Tests of malfunctions on the install's AKM, each given outright on its shot.
- `mounting.rs` (337): Tests of mounting with the install's body animator, on a sill set before the character.
- `no_chamber.rs` (102): Tests of a weapon with no chamber, which fires straight off its box.
- `panels.rs` (79): Tests of what the battle screen's ammunition panel is told by the hands.
- `pellets.rs` (65): Tests of a shotgun's pellets against the game's, from a recorded shot.
- `poison.rs` (191): Tests of a poisoning as the install has it, from the blow to the cure.
- `posing.rs` (353): Tests of the animators' split into the bones each controller follows and its clips in two.
- `quick_draw.rs` (463): Tests of the quick put-away of a firearm for the holster's weapon.
- `room.rs` (171): Tests of the room to move, with the install's movement and body animator.
- `searchable.rs` (49): Tests of which of the install's things hide what they hold until searched.
- `sights.rs` (360): Tests of the sights as the game picks them, on the stock builds of the range that carry them.
- `slopes.rs` (120): Tests of slopes, sliding and the sprint's slide, with the install's movement and body animator.
- `standing.rs` (74): Tests of the character as the game's own screens stand it.
- `steps.rs` (147): Tests of the sounds of moving as the game's rules ask for them.
- `stimulators.rs` (226): Tests of stimulators' and buffed foods' sets of buffs as the install has them.
- `tactical.rs` (151): Tests of the tactical devices' keys as the app and the replay run them.
- `tactical_reload.rs` (51): Tests of the weapon bone held toward its rest by the tactical reload's blend.
- `third_person.rs` (397): Tests of a character seen from outside, measured with the real install's data and the stock AKM.
- `time_of_day.rs` (281): Tests of Customs' time of day as the install sets it up, beside a recorded scene of it.
- `unique_animations.rs` (217): Tests of a part with its own animations, on each weapon whose animator goes by one.
- `zeroing.rs` (314): Tests of zeroing on the range's stock builds.
- `zoom_ring.rs` (112): Tests of a variable scope's ring turned by its zoom.

### crates/trp-sim/tests/sim/figure

- `mod.rs` (59): Tests of the inventory's OVERALL figure against the game's, from the install and three recorded takes.
- `held.rs` (207): What the figure holds: grenades, knives, the rangefinder and the AKM's moves.
- `idle.rs` (122): The figure's events and kinds, the rifle held at idle and the look out.
- `takes.rs` (315): The figure run against the three recorded takes.

### crates/trp-sim/tests/sim/grenade_knife

- `mod.rs` (50): Tests of a grenade's hands and a knife's on the install's own animators.
- `blade_strokes.rs` (387): A blade's strokes on the body, their marks and the strokes seen as recorded.
- `first_person.rs` (223): Grenades drawn and thrown, knife strikes, and a rig at rest under the eye.

### crates/trp-sim/tests/sim/hands

- `mod.rs` (70): Weapon motion measured with the real install's data and the stock AKM.
- `bob.rs` (160): The walk bob rewound, the lame walk's pitch and the first step's tempo.
- `breath.rs` (252): Holding the breath, tiredness and the out-of-breath heave.
- `disturbances.rs` (232): Hits, blasts, the load, the kit's ergonomics, damaged arms and the hip penalty.
- `obstacles.rs` (356): The arms' solve, walls turning the weapon away and the ground under the camera.
- `shots.rs` (210): Shots kicking the weapon and the camera, automatic fire and the reload's length.
- `sway.rs` (239): Breathing at rest, mounts, turning sway, the walk bob and the time to aim.

### crates/trp-sim/tests/sim/install

- `mod.rs` (100): Movement measured with the real install's data.
- `containers.rs` (296): The hands working a container, examining from the world and searching.
- `controls.rs` (260): Commands, slow leans, looking a weapon over, and the mouse bindings.
- `doors.rs` (192): Kicking a door and the door's menu.
- `empty_hands.rs` (179): Empty hands read like a weapon, and fists in a sprint.
- `equipment.rs` (384): Starting equipment, the equipment panel, crates and recorded equipment lists.
- `falls.rs` (313): Falling off ledges, foot motion, crouched walks and the ribcage lag.
- `hand_actions.rs` (364): The Model 700's bolt and magazine and the Five-seveN's slide lock.
- `legs_dry.rs` (201): Sprints that run the legs dry.
- `m3.rs` (261): The M3 pumped and filled a shell at a time, and its folding stock.
- `magazines.rs` (402): Magazines and rounds moved from the inventory, quick reloads and magazine data.
- `own_weapons.rs` (477): Weapons of one's own built and worked on a donor's rig and animator.
- `prone.rs` (258): Lying down, crawling, getting up and a reload holding a lying body still.
- `screens.rs` (189): Screens and the inventory taking the keys, and the lean keys.
- `vaults.rs` (334): Vaults and climbs in a yard of walls and boxes.
- `view.rs` (340): The view hung from the ribcage and the view and body turning evenly.
- `walking.rs` (478): Walking speed, sprint, jump, lean, inertia and landing against the install's tables.
- `weapon_animator.rs` (261): The weapon's animator as a state machine, and head gestures.
- `weapon_build.rs` (260): Weapons built into slots, weapon skills, sight lines and parts on bones.
- `weight.rs` (43): Tests of the weight's notices against its limits.

### crates/trp-sim/tests/sim/kinds

- `mod.rs` (91): Tests of one weapon of each further kind, each run on its own animator from the install.
- `actions.rs` (244): Each kind of action: bolts, self-loaders, revolvers, barrels and the launcher's drum.
- `by_hand.rs` (432): Rounds moved by hand from the inventory, the unload keys and chambers filled outright.
- `chambers.rs` (221): Rounds and cases shown in chambers of their own, and the rocket launcher.
- `revolvers.rs` (224): A cylinder's quick reload, the fistful and the cylinder held and turned through shots.

### crates/trp-sim/tests/sim/puppet

- `mod.rs` (73): Tests of a character no one is at the keys of, with the stock AKM, from the install.
- `colliders.rs` (343): Collider arms, threads of their own, the bound around colliders and ragdolls.
- `drawing.rs` (167): Frames drawn between a puppet's steps.
- `economy.rs` (376): The posing economy: scenes, makings, poses asked for and what it saves.
- `walking.rs` (313): Puppets standing and walking, capped by hits and legs, with load inertia and skills.

### crates/trp-tools/src/bin/probe

- `main.rs` (215): Inspection tool for the install's bundles.
- `animators.rs` (279): Animator controllers: states, parameters, the parameters' use and root motion.
- `body.rs` (330): The body animator's states and clips, outfits' fittings and skins.
- `bundles.rs` (204): A bundle's keys, objects, names, dumps, scripts, trees and textures.
- `clips.rs` (277): Animation clips and the blend trees checked against them.
- `custom.rs` (211): Custom weapons and a weapon drawn with a model over it.
- `fonts.rs` (287): TextMeshPro's fonts and texts: scales, widths, captions and materials.
- `icons.rs` (313): Items' icons, their materials and bounds, and an item's template.
- `level.rs` (385): A level's scenes, props, clouds, colliders, ballistics, lights, layers and collisions.
- `other_formats.rs` (320): Another game's OGF models and XDB archives, and FBX files.
- `panels.rs` (415): The inventory's panels as the app reads them: health, scene panels, battle, quick, stash, tabs, item views.
- `particles.rs` (155): Particle systems and every weapon's muzzle effects.
- `player_settings.rs` (219): The key bindings, the player's scripts, the engine, the hard settings and the ragdoll.
- `prefabs.rs` (421): Prefabs, resources and sprites found by name in the bundles.
- `printing.rs` (199): Printing JSON, object trees, transitions and props' levels of detail.
- `scene_objects.rs` (456): A build's serialized files and scene objects: classes, pointers, fields and raw bytes.
- `shaders_textures.rs` (344): Shaders, compute shaders, BC7 textures and their statistics.
- `sounds.rs` (302): The sounds of the interface, things, drops, steps, hands and aiming.
- `voices_mixer.rs` (185): The voices, Vorbis clips and the master mixer.
- `weapon_parts.rs` (282): Folding sights, slot bones, tactical devices and sights.
- `weapons.rs` (352): Weapons as assembled: presets, templates, grenades, knives, colliders, parts' states and binds.
- `windows.rs` (407): The game's windows and small screens: info, tag, context menu, notice, repair, split, skills, mastering.
