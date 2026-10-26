# Tactical devices: weapon lights, lasers, combo units, the head light

Read 2026-10-24 (session 127) from the decompiled game and the install through `probe`; the reading's
notes and outputs: `local\tactical_s127.md`, `local\tactical_s127\`. Our own words. Conventions:
Unity's (y up, z ahead); "normalised" animator times are fractions of a state's clip.

## The items

**Three item classes carry a light** (`LightComponent`): `Flashlight`, `TacticalCombo` and `LightLaser`.
The component keeps two saved figures, `IsActive` and `SelectedMode`; a mode written is wrapped by the
template's `ModesCount` where that is above nothing. A request names a device, on or off, and a mode.

**The install has 24 devices**: 3 flashlights, 21 combo units, no `LightLaser` item. What each mode shows
(white light, a visible laser red, green or blue, an IR laser, an IR light, the RAPTAR's rangefinder):
`local\tactical_s127.md`, its table. E.g. the SureFire XC1, one mode, light; the X400, three: light, light
and red laser, red laser; the AN/PEQ-15, four: red laser, IR laser, IR light, IR light and laser; the
Steiner DBAL-PL, six. **No default preset carries one**; 40 other presets do ("M4A1 2017 New year": an
X400 and a PEQ-15). **There is no head lamp item**: any device fitted to headwear (the 6B47 and the DevTac
face covers' `mod_equipment`, the LShZ-2DTM's `mod_mount`, the Ops-Core rail and clamp adapters on the
FAST and others) is the head light.

## The keys

| Game key | command | the install's binding | press type |
|---|---|---|---|
| `Tactical` | `ToggleTacticalDevice` | T | press |
| `ReleaseTactical` | `ReleaseTactical` | T | release (made by the settings from `Tactical`'s binding while it is a press) |
| `NextTacticalDevice` | `NextTacticalDevice` | Ctrl+T | press |
| `ToggleHeadLight` | `ToggleHeadLight` | H | release |
| `SwitchHeadLight` | `SwitchHeadLight` | Ctrl+H | release |
| `ToggleTacticalInteraction` | `ToggleTacticalInteraction` | T+B | press |

**The setting** `TacticalInputMode` is Press (the default) or Hold; T+B flips it, saves it and raises the
HUD's panel (below).

**A weapon's keys act on all its devices at once**: every light component in the weapon's slots, nested
parts included.
- **Press mode, T**: each device's on is flipped on its own (one on and one off swap), the request animated.
- **Hold mode, T down**: where not all are on already, all on, not animated, and the player's tactical
  sound where accepted. **T up** (Hold mode only): where any is on, all off, forced past any refusal, not
  animated, the sound. Hold mode's lights also go off, forced, with the sound, when the hands change, on a
  death, on any screen but the battle screen (the inventory), on the window's focus lost, on quitting and
  on the voice chat starting.
- **Ctrl+T** (either mode): every device's mode one up, each wrapping by its own count (an off device's too,
  unseen; a one-mode device stays at 0), animated.

**The path**: the request goes to the firearm controller (`SetLightsState`), which, unless forced, asks
the operation in hand whether it may (`CanChangeLightState`); then each device's two figures are written,
the hands animator is given its trigger `ModSwitch` where animated, the device's model is switched **at
once** (`UpdateBeams`), and the weapon's hip penalty is worked out again (below). Accepted is true even
where nothing changed.

**Refused only while** the weapon is drawn or put away, a part is fitted or taken off, a magazine is put in
or pulled out through the inventory, the backpack is dropped, or the bipod is toggled. **Allowed** at rest
(so sprinting, aimed, crouched, prone), in any fire, in the R key's reload, in a malfunction; nothing in the
movement's state refuses it.

## The animation and the sound

**`ModSwitch`** (the trigger a scope's mode change gives too): only the hands' `IDLE` state has a
transition on it, to `MOD SWITCH` (0.33 to 0.40 s by weapon; the M4A1's 0.40 s, out at 0.9 of it), back to
`IDLE` at its exit. Nothing resets the trigger, so a toggle made away from `IDLE` waits for it, the light
having changed at the press; **but one made in automatic fire is lost**: as the burst ends the hands
start into `MOD SWITCH` and, the next frame, as the fire's operation gives way to idling and resets its
triggers, the transition is dropped and the hands stay at `IDLE`, with no switch and no sound (recorded:
course 7, the M60E6's `05-refusals`, the toggle at 8.506 s, the transition begun at 9.087 s and gone at
9.098 s). In a reload or a sprint's own state: not recorded yet. The clip's sound event `ModSwitch` plays
the weapon sound player's `SndModSwitch` (one of `weapon_light_switcher1` to `3`, 0.23 s, rolloff 30), at a
share of the clip by weapon (the M4A1's 0.45, the AKM's 0.261). No other animator parameter concerns the
devices. **As recorded**, from the press at rest: the M60E6 FDE fully in `MOD SWITCH` after 0.067 s, out of
it from 0.175 s, at `IDLE` by 0.192 s, its sound at 0.075 s; the M4A1 in by 0.067 to 0.075 s, out from
0.362 to 0.367 s, back by 0.400 to 0.408 s, its sound at 0.183 to 0.187 s.

**The player's tactical sound** (Hold mode's toggles and the head light): the movement's
`SwitchHeadlights`, `weapon_light_switcher1`, in the world 1.2 m above the feet, the character's group,
rolloff 5.

## The look

**Which objects are on** (`TacticalComboVisualController`, on every model of a thing with a light): below
the device, in breadth-first order, every object named with `mode_`: `mode_000` is shown while off, mode
n's object (`mode_00<n+1>`) while on in mode n, and nothing else. Every light, laser, LED, switch mesh,
glass and flare is a child of those. **No difference between first and third person** for a weapon's
devices.

**The lights**: every white or IR main light is a spot, soft shadows at medium resolution, near plane 0.1,
with a cookie (`weapon_flashlight_cookie`, white; `FlashlightCookie`, IR). The figures by device
(`local\tactical_s127.md`, 4.2): the XC1 white, 1.5, 30 m, 65 degrees (6 inner); the X400 0.81/0.91/0.99,
1.2 to 1.3, 38 to 42 m, 85; the Klesch-2U 2.4, 35 m, 105; the PEQ-15's IR 1.55 to 1.65, 95 to 100 m, 25. An
LED is a point light of 1 cm reach (it lights only itself). **A light is drawn at its distance fade's own
figure** (`CullingLightObject` on its object: `_useLightIntensityFromEditor`, set on every device, takes
`_maxLightIntensity` in place of the light's own), times the fade by the camera's distance, 1 for one's own:
the WF-501B's two lights both at 1 (their own 1.2 and 1), the Klesch-2P's both at 1.2 (1.4 and 1.2), the
X400's at 1.2 and 1.3 (recorded: the WF-501B's at 1 on every lit frame). A light lit for the first time is
at nothing for one frame (recorded, not built). The WF-501B's mode 0 is its `light_001` (60 m), mode 1 its
`light_002` (51 m) (recorded). One's own weapon casts **no shadow in its own lights**
(`GunShadowDisabler`), nor in its head light's; a white light has a light shaft (`VolumetricLight`,
below) where the graphics option `VolumetricLight` is on (the component destroys itself as its object
wakes where off), and a lens flare (below) seen only looking into it.

**The laser** (`LaserBeam`, on each beam object, its z the device's axis), each frame after the animation:
one ray from 0.1 m ahead, 100 m long, against the terrain, the high-detail colliders, the hit colliders and
the dead. **With a hit** at distance d (k = d/100): the beam drawn d + 0.1 long at brightness 1 − k; the dot
a quad lying on the surface, its size lerp(0.017, 0.2, k), at the same brightness; a small spot light 0.1 m
off the surface (toward the normal less the beam), reach 0.2 m, intensity 3.1(1 − k), its cone lerp(15,
300, k) degrees (the engine caps it at 179). **With none**: 100 m long at full brightness, no dot, the light
at nothing. **`Custom/LaserBeam`**: additive, a camera-facing ribbon 4 mm wide, four times as bright at the
emitter, shimmering by a 3D noise; **`Custom/LaserPoint`**: an additive dot sized by the camera's field of
view and its distance. Each is times `_Factor`, or `_IkFactor` while night vision is on, and nothing under
thermal vision: a visible laser is 80 times as bright through night vision, an IR one (`_Factor` 0) is
seen through night vision only. **IR lights** light the world only in a night vision view (`IkLight`: no
intensity for a camera without it).

**The head light** (`Player.SwitchHeadLights`, from the movement's keys, either input mode): refused while
its lock is on, while the hands may not act (sprint, crawl, the doors' states), or with no head device.
H flips each head device's on, Ctrl+H adds one to its mode, **written on the item at the press**; then the
gesture `HelmetRailGear` (the body's `15 Helmetrail`, 1.00 s) is asked, refused as any gesture is (a
sprint, the hands not at rest). Whether or not it played, **at 0.8 s** the sound and the model's switch,
**at 1.4 s** the lock lifted. **In first person** (one's own player, any point of view): the head device's
meshes are hidden for good, and each of its lights and lasers is placed every frame at the camera, 0.2 m
to its side (right or left by where it sits), up and ahead, turned 10 degrees down and 3 in, plus a term
below a field of view of 50. Only a light that is on is moved, each along its own turn of the frame it was
last moved, so one lit for the first time moves along its model's turn on that frame. **As recorded**
(course 7, the M60E6's `04-head-light`, a WF-501B on the left of the helmet): the switch 0.800 to 0.804 s
after the press and the lock lifted 1.401 to 1.411 s after it, each on the frame our coroutine gives, the
first wait counted from the press's frame and the second from the switch's; the gesture's flag
(`IsInteraction`, `InteractionIndex` 15) up 1.008 to 1.012 s; the item's state written at the press, the
light shown at the switch.

## The light shaft and the flare

Read 2026-10-25 (session 128) from the decompiled game, the install and the published source of the
shaft's origin; the notes and outputs: `local\flare_shaft_s128.md`, `local\flare_s128\`.

**The shaft** is SlightlyMad's published `VolumetricLights` (BSD licence) as the game changed it: the
components destroy themselves where the graphics option is off; a light under 0.001 is not drawn; a
weapon's lights keep their shadow at any distance; the composite comes after the transparent things and
under thermal vision none. Per white spot light: its 16-sided cone drawn into a quarter-resolution buffer
(half with an upscaler on); along each view ray inside the cone, 16 samples between where the ray enters
and the nearer of where it leaves or the scene, offset by an 8 by 8 dither; each sample the cookie's alpha
times the engine's distance fall-off times one comparison with the light's shadow map times 0.09 per step
times a phase of Henyey-Greenstein with g 0.014 (all but flat); extinction, noise and height fog off; the
sum times the light's colour and intensity; the depth halved by a checkerboard of its least and most, a
13-tap blur that keeps to the depth both ways, an upsample that keeps to edges past 1.5 m, and the shaft
added to the picture before the bloom and the tonemap: a faint haze, about 0.036 of the light's colour
over 5 m. **The user's game has the option off**: in course 7 the component is on a light only until it
is first lit (the WF-501B's `light_002` before 4.31 s, then gone), so no shaft is drawn there.

**The flare** (`FlareLight` on each mode's `NewMultiFlareLight`, drawn by one manager for every light of
the scene): screen-aligned quads at the light's place, moved 0.1 m toward the camera, from a 1024-square
atlas, after the transparent things; their size an angle; their brightness by the distance and by how
much of 23 depth taps around the light are clear. `SingleFlareController` moves its alpha at 2 a second
(15 for a laser) toward `(dot(toward the camera, the light's way) − 0.6) / 0.4` (0.999 for a laser), not
held at nothing: **behind one's own light the alpha sinks below nothing and no flare is ever drawn**, so the
flares are another's lights' (a bot's), and wait for one.

**The HUD** (`TacticalInteractionPanel`) shows only on T+B: "Press" or "Hold", lower right, text size 50.
Nothing shows on a toggle or a mode change.

## What it does to the weapon

**The hip penalty** (`UpdateHipInaccuracy`, as a weapon is drawn, changed or a request accepted): E the
weapon's total ergonomics; where any **combo unit** on it is on (a light-only XC1 too; a `Flashlight` does
not count), 0.3 − E/400, else 1 − clamp01(E/250 − 0.15); written into the breath's `HipPenalty`, the hip
wander's scale (weapon-handling spec). Worked out when the controller is made (a draw) and as a request is
taken; the game never keeps the prefab's 1 once a weapon is drawn (E 50 gives 0.95, E 100 0.75). **As
recorded** on every change of five weapons (`courses6`: the M60E6 FDE at E 4.75, 1 and 0.288125; the M4A1
at 45.57 and 47.53, 0.96772, 0.186075 and 0.95988; the MDR at 76.26, 0.84496; the SPEAR at 30.69 and 29.76,
1), the rule to the last figure. **Ours** (0.120.0) on the simulation's total ergonomics, which lacks the
gear's ergonomics penalty: the M60E6 build's 5 against the game's 4.75 (0.2875 for 0.288125).

## State

On the item (`upd.Light`), no battery and no timer. In Press mode only the keys change it: it stays through
weapon switches, the inventory and saving, and a weapon drawn comes out with its device on; by the code a
slung weapon's light stays on (to be seen in a recording). In Hold mode, off at every event above.

## Not read, or not settled

How T, Ctrl+T and T+B are told apart by the input (our key manager takes the longest combination; whether
the game's T alone also fires with Ctrl held is not read); the clips' motion (`tp_helmetrail`); the order
of the IR lights' switch against the recorder's frame, and of the lasers against the head light's placement
(a helmet laser may lag a frame); the HUD panel's own fade (ours the optic panel's: 0.5 s in, 3 s, 0.5 s
out); which of the two spots of the X400, the Klesch-2P and the DBAL is which mode's (the WF-501B's:
recorded, above); whether a reload's change of magazine works the hip penalty out again
(`WeaponModified`); the one frame at nothing of a light first lit; the flare's and the shaft's figures not
listed above (`local\flare_shaft_s128.md`, "Not read, or uncertain").

## Built

0.120.0 (2026-10-25), behind the options' `TACTICAL DEVICES` (`SIMPLE`, restart required):

- **The devices** (`trp_data::tactical`): every part of a build with a light, its class and modes, its
  model's `mode_*` objects, each light's figures with its distance fade's intensity, cookie, infrared
  flag and shaft figures, each laser's figures with its two materials and pictures (the 3D noise read as
  a `Texture3D`); a head device read from its own prefab (`read_alone`). `probe devices <preset, its id
  or a key, or a device's template>` prints them.
- **The state and the keys** (`trp_sim::tactical`, `tactical_view`): on and mode on the item
  (`inventory::Light`); T, Ctrl+T in Press and Hold modes, Hold mode's lights let go with the hands, a
  death, the inventory and the window's focus; T+B the mode flipped and kept (the GAME settings'
  `tactical_input`, the install's `TacticalInputMode` until changed) with the HUD's panel (`sight_panels`,
  the scene's `TacticalInteractionPanel`, "Press" or "Hold"); the firearm's refusals
  (`Firearm::can_change_lights`); the hands' `ModSwitch` and its sound through the clip's event; H and
  Ctrl+H with the gesture (`Interaction::HelmetRailGear`, the body's 15), the coroutine's 0.8 s and 1.4 s
  and the lock; the player's tactical sound (`TOGGLE_SOUNDS`' `SwitchHeadlights`) for Hold mode and the
  head light, played whole (`PROVISIONAL`: the game's is placed 1.2 m over the feet, rolloff 5).
- **The hip penalty** by ergonomics for every weapon (`Hands::update_hip_inaccuracy`), lower with a combo
  unit on.
- **The look**: each device's model by its state (`mode_000` off, its mode's object on), with the switch
  off too; its lights as Bevy's spot and point lights with the cookie (`SpotLightTexture`; the game's
  shading reads its alpha over the cone's square, `game_shading.wgsl`), soft shadows by Bevy's filtering,
  placed on their objects each frame (the weapon's, as Unity keeps the turn) or by the camera (the
  head's, `head_pose`); one's own hands' rigs on a layer of their own (`weapon::HANDS_LAYER`), out of the
  devices' lights' shadows; the lasers (`laser.wgsl`, in the effects' pass): the ray against the range
  and the dummies, the beam and the dot by the game's shaders' maths, the dot's light; infrared lights and
  lasers dark (no night vision here). Since 0.121.0, with `CUSTOMS LIGHT` off, a light is multiplied by
  2^9.7 for the camera's own exposure (`tactical_view::exposure_gain`; `lighting.md`, "The range's own
  light by the hour"): until then it was worked out for the game's exposure and barely showed.
- **Not built**: the light shaft (the user's game has it off; to come with a graphics option of its own);
  the flare (never drawn for one's own lights: with a bot's); the one frame at nothing of a light first
  lit; a slung weapon's lights.

## Compared with

`courses6` (recorder 0.13.0, 2026-10-24), course 7's six scripts with the M60E6 FDE (a Klesch-2P and an
X400, a WF-501B on the helmet) and with the M4A1 (an Olight Baldr Pro; its `05-refusals` from 2.8 s and its
`06-modes-off` with the SPEAR, the script having drawn the first primary), by `cargo run -p trp-sim --bin
tactical -- <folder>` (0.120.0; `local\compare_c7_m60_s128.txt`, `compare_c7_m4_s128.txt`):

- **Every press** (29 with each weapon): the devices' states our keys give from the
  recorded states before it, refused or not by our rule on the recorded operation, are the recorded ones
  after it, every one (a toggle in the put-away refused, one in automatic fire taken).
- **The hands**, our firearm on the recorded frame times: for every press at rest (115 moments with the
  M60E6, 65 with the M4A1) `MOD SWITCH` entered, fully in, left, back at `IDLE` and its sound on the
  game's own frame; the toggle in automatic fire, the burst driven, begun 0.581 s after the press and
  dropped as the game's.
- **The head light**: the five switches and five locks lifted on the game's frames; on all 1,189 lit
  frames but two its light within 1.5 mm of the game's (0.4 mm rms) and its way to the thousandth of a
  degree (the two left out: a light lit for the first time); its figures (intensity 1, 60 or 51 m, 65
  degrees, soft shadows, `weapon_flashlight_cookie`) the game's, and which light each mode is.
- **The hip penalty**: on every change, the rule on the recorded figures to the last digit.
- **Not compared**: the weapon's own lights, their models' objects and the lasers (recorder 0.13.0 looked
  for them under the player only, and the weapon in hand is a scene root of its own: recorder 0.13.1
  looks there too, its course 7 to be run again); the dark range, the laser's dot near and far, Hold mode
  and the lock by hand (course 7's steps made by hand were not run); a capture beside the game's
  pictures in daylight only (`local\shots\c7_m60_mode1_s128.png` beside the game's
  `01-toggle-pic2.png`: the two faint beams leaving the two devices).
