# Features

What TRP builds, one line each. **Spec** names the document under `docs/spec/` that says how the game behaves. **Switch** is the feature's key: `--feature <key>=off` on the command line, or the same key under `switches` in the options file, turns the system off at the next start.

Only seven switches are on a menu page: SIMPLE XP GAIN, BENCHES, CLOTHING, SMOOTH ANIMATIONS, GROUNDED FEET and LEDGE CLIMBING on FEATURES, and COMPATIBLE ITEMS HIGHLIGHT on HUD. LEDGE CLIMBING is not built: its row is dim and cannot be turned on. The rest are kept for hosts and for testing.

## Character

| Feature | What it does | Spec | Switch |
|---|---|---|---|
| Movement | Walking, sprinting, stances, leaning, jumping, sliding and lying down, with the character's own collision. | `movement.md` | `movement` |
| Vaulting | Vaulting over and climbing onto what stands ahead. | `vaulting.md` | `movement` |
| Ledge climbing | Not built: taken out in 0.259.0. Its switch stays on FEATURES, dim, and cannot be turned on. | `settings.md` | `ledge_climbing` |
| Stamina | Legs, arms and breath drain as you act. | `stamina.md` | `stamina` |
| Weight | What you carry slows you, adds inertia and costs stamina. | `movement.md`, `stamina.md` | `weight` |
| Health | Seven body parts, bleeding, fractures, pain, hunger, thirst, poisoning and death, with medicine, food and stimulants. | `health.md` | `health` |
| Armour | What a hit does to the body and to what it wears. | `body-hits.md` | `ballistics` |
| Skills | Skill levels and weapon mastery take effect. | `skills.md` | `skills` |
| Experience | Skills, mastery and the character's level grow from what you do. | `skills.md` | `experience` |
| Simple XP gain | Every action earns its full experience. Not Tarkov's rule. | `skills.md` | `simple_xp_gain` |
| Free look | The head turns apart from the body. | `free-look.md` | |
| Gestures | Hand signs and voice phrases. | `gestures.md` | `gestures` |
| Character body | The body in first person, the wrist watch, worn gear and slung weapons. | `body-animator.md` | `character_body` |
| Head fitting | What is worn on the head takes the form the game gives it beside what else is worn: a headset's band over a cap, a hood up under headwear, the head drawn in under a face cover. | `body-animator.md` ("The head's things fitted to one another", "The head's own mesh under a face cover") | `head_fitting` |
| Other characters | A character seen from outside, with levels of detail by distance. | `third-person.md` | |

## Weapons

| Feature | What it does | Spec | Switch |
|---|---|---|---|
| Weapon handling | Firing, reloading, checks, fire modes, drawing and holstering, for every kind of weapon in the install. | `firing.md`, `pistol.md`, `shotgun.md`, `bolt-action.md`, `more-weapons.md` | `weapon_handling` |
| Weapon motion | Sway, inertia, breathing and recoil. | `weapon-handling.md` | `weapon_motion` |
| Arms | Both arms solved onto the weapon. | `hands-on-weapon.md` | |
| Attachments and sights | Weapons built from their parts, with scopes that show a magnified picture. | `optics.md` | `attachments` |
| Ballistics | Bullets fly, penetrate and ricochet, and hits leave holes, dust and sound. | `ballistics.md`, `body-hits.md` | `ballistics` |
| Weapon condition | Wear, malfunctions and overheating. | `malfunctions.md` | `weapon_condition` |
| Weapon stances | Blind fire, mounting, bipods and the left shoulder. | `blind-fire.md`, `mounting.md`, `left-shoulder.md` | `weapon_stances` |
| Tactical devices | Weapon lights, lasers and helmet lights. | `tactical-devices.md` | `tactical_devices` |
| Underbarrel launcher | The GP-25 and the M203. | `underbarrel-launcher.md` | `weapon_handling` |
| Grenades and knife | Throwing grenades, and slashing and stabbing. | `grenades-and-knife.md` | `grenades_and_knife` |
| Rangefinder | The handheld rangefinder and its readout. | `rangefinder.md` | |
| Custom weapons | A weapon that is not the install's, on another weapon's animations. | `custom-weapons.md` | |

## Inventory and world

| Feature | What it does | Spec | Switch |
|---|---|---|---|
| Inventory | Gear slots, containers, moving items and quick slots. | `inventory.md` | `inventory` |
| Inspect window | A thing's figures and its model. | `inspect-window.md` | `inventory` |
| Looting | Unknown items are examined, and crates and bodies are searched. | `inventory.md` | `looting` |
| Raid limits | Tarkov's limits on what you may carry or discard in a raid. Off by default. | `inventory.md` | `raid_limits` |
| Hide unfinished items | Items SPT never hands out are left off the range. | `inventory.md` | `hide_unfinished_items` |
| World interactions | Doors, containers and items on the ground. | `interactions.md` | `world_interactions` |
| Physics | Corpses, and things dropped and thrown, on PhysX. | `physics.md` | `world_interactions` |
| Targets | Dummies and steel poppers on the range. | `range-targets.md`, `body-hits.md` | `targets` |
| Benches | A gun bench and an armour bench that open the edit build screen. | `workbench.md` | `benches` |
| Clothing | Any suit from the OVERALL tab, and outfits read from S.T.A.L.K.E.R. | `outfits.md` | `clothing` |

## Presentation

| Feature | What it does | Spec | Switch |
|---|---|---|---|
| Tarkov UI | Screens drawn as the game's own, from the install. | `inventory.md`, `settings.md` | `tarkov_ui` |
| Item icons | Item pictures rendered from their models and kept between runs. | `icons.md` | `item_icons` |
| Sounds | Footsteps, weapon handling, gunshots, voices and the world's sounds. | `sound.md`, `footsteps.md` | `sounds` |
| Headgear hearing | Helmets and masks over the ears muffle what you hear. | `sound.md` | `headgear_hearing` |
| Visual effects | Muzzle flash, smoke, explosions, tracers, cases and screen effects. | `particles.md`, `firing.md` | `visual_effects` |
| Face shield cracks | Hits on a lowered face shield show as cracks in its mask. | `body-hits.md` | `face_shield_cracks` |
| See-through glass | Face shields and window magazines are seen through. | `inventory.md` | `see_through_glass` |
| Tarkov lighting | Customs' sun, sky and clouds through the game's camera and shading. Off by default. | `lighting.md`, `ambient-occlusion.md` | `tarkov_lighting` |
| Texture streaming | Pictures loaded by level of detail. | `streaming.md` | |
| Smooth animations | Hands and weapon animations drawn on smooth curves between frames. | | `smooth_animations` |
| Grounded feet | Feet drawn on the ground where Tarkov lets them float. Off by default. | | `grounded_feet` |
| Compatible items highlight | What fits the item under the mouse glows. Off by default. | | `compatible_items_highlight` |

## Options

The options tab has six pages: GRAPHICS, SOUNDS, HUD, CONTROLS, FEATURES and SKILLS. Its rules are in `settings.md`. Every setting is kept in `local/options.json` and most are taken at once. A setting that needs a restart says so on its row.

## Not built

- The world's ambient sounds.
- STALKER as a host: no value is taken from a host game yet.
- Co-op.
