# Tactical devices: weapon lights, lasers, combo units, the head light

How the weapon's lights, lasers and combo units and the head light are switched and drawn. Read 2026-10-24 from the decompiled game and the install through `probe`; the reading's
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
  on the voice chat starting: each an event, once (`GamePlayerOwner.ReleaseTactical`), the hands' change
  as it begins (`Player.HandsChangingEvent`, the old weapon still in hand), the hip penalty worked out
  again on that frame (recorded: `courses8\tactical-hold`, the inventory at 5.777 s and the switch's
  `Remove` at 9.903 s, the lights off and the penalty back on each one's frame).
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
meshes are hidden for good, and each of its lights and lasers is placed every frame (in `Update`, before
the camera's own move: by the camera as the frame before left it, a frame behind the view) at the camera, 0.2 m
to its side (right or left by where it sits), up and ahead, turned 10 degrees down and 3 in, plus a term
below a field of view of 50. Only a light that is on is moved, each along its own turn of the frame it was
last moved, so one lit for the first time moves along its model's turn on that frame. **As recorded**
(course 7, the M60E6's `04-head-light`, a WF-501B on the left of the helmet): the switch 0.800 to 0.804 s
after the press and the lock lifted 1.401 to 1.411 s after it, each on the frame our coroutine gives, the
first wait counted from the press's frame and the second from the switch's; the gesture's flag
(`IsInteraction`, `InteractionIndex` 15) up 1.008 to 1.012 s; the item's state written at the press, the
light shown at the switch. **Recorded again** (`courses2\6-helmet`, recorder 0.10.0, a WF-501B on the Galvion
Caiman's rail; the key itself not recorded): eight gestures, four of them walking, each 1.008 s (one 1.010),
and `weapon_light_switcher1` 0.800 s after each began; the gestures replayed as begun, every frame the game's;
the devices not run there (the replay has none where a recording lists none: inventory spec, "A switch on the
head").

**Seen from outside, and put away** (read 2026-11-08). The head's lights and lasers are moved to the
camera by the player's owner alone (`GamePlayerOwner.LateUpdate` calls `HeadLightsValidator`, which holds the
head's lights from `ParentedModelChangeHandler`); another player's stay where their model has them, on the
device on the helmet, turned as the helmet is, its meshes drawn and its `mode_*` objects switched as any model's.
The hideout, the one place the game shows its own player from outside, pauses the moving there
(`HideoutPlayerOwner`: `SetPause(!FirstPersonMode)`). A view from outside shows the character as others see it
(`third-person.md`), so there the head's lights, lasers and flares are on their objects on the model of what is
worn. **A weapon put away** hangs its whole model on its slot's bone (`PlayerBody.SlotView.CreateAndParent`, the
body animator spec's "Weapons on the body"); for its own player the game switches that model's renderers off and
nothing else, so its devices' objects, lights and flares stay as its item's state has them, and its lasers too
(`LaserBeam.LateUpdate` draws them itself, `Graphics.DrawMesh`, no renderer of the model). A slung rifle's light
so shines from the back, down and behind the character, in first person as from outside.

## The light shaft and the flare

Read 2026-10-25 from the decompiled game, the install and the published source of the
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
over 5 m. **The recorded game has the option off**: in course 7 the component is on a light only until it
is first lit (the WF-501B's `light_002` before 4.31 s, then gone), so no shaft is drawn there.

**The flare** (`FlareLight` on each mode's `NewMultiFlareLight`, drawn by one manager for every light of
the scene): screen-aligned quads at the light's place, moved 0.1 m toward the camera, from a 1024-square
atlas, after the transparent things; their size an angle; their brightness by the distance and by how
much of 23 depth taps around the light are clear. `SingleFlareController` moves its alpha at 2 a second
(15 for a laser) toward `(dot(toward the camera, the light's way) − 0.6) / 0.4` (0.999 for a laser), not
held at nothing: **behind one's own light the alpha sinks below nothing and no flare is ever drawn**, so in
first person the flares are another's lights' (a bot's, the dummies' here); seen from outside, one's own light
faced shows its flare as another's would.

**A light's flares** (read again 2026-11-08 with the install: `probe devices <device>` lists them). On each mode's
`NewMultiFlareLight`, at the lens with the light's turn (its z the beam's axis): `FlareLight`'s own size and alpha
for all its flares (`_totalScale`, 1.2 to 2.1 by device; `_totalAlpha`, which the controller replaces each
frame), and its flares, each with a size both ways, an alpha, a colour, a kind (`FlareType`: 0 normal, 1 off
screen, 2 noise, 3 flashlight), a near and a far distance and its size and alpha at each, and a picture's number
in the level's atlas. A white light has four: three flashlights (a hot point with a very wide faint halo, size 2;
a hard disc, alpha 0.1 to 0.3; a thin many-rayed star) and one noise flare (the star again, size 2, alpha 0.2 to
0.3, out by 15 m); the Klesch-2U's light 1.5. An infrared light's flares (`IkLight.FlareLight` names them) are put
to −1 in every view without night vision, so never shown here. A light enters the drawn set while its object is
on: switched on, its alpha starts from nothing and climbs at its speed (2 a second for a white light).

**Each flare's figures** (`PrepareGpuData`, for a light on): its colour the flare's times the light's alpha, its
own least and alpha and the batch's alpha factor, held within 0 and 1, the light's alpha itself kept beside it;
its size the flare's times its least size, the light's size and the batch's size factor; its distance curve's
start, one over its length, and its size's and alpha's change over it as shares of their nearest. **The
flashlights crowd** (`ManageFlareOverlap`, their batch's alone, not under night vision): each one's size times
`lerp(1, 0.25, n / 5)`, `n` the sum over the other flashlight flares within 0.5 m of one less their distance over
0.5, at most 5; a lone light's three at one point count 2 each and draw at 0.7. **The eyes' protection**
(`FlareBlindnessController`, `Player.GetBlindnessProtection`): the largest `BlindnessProtection` of the armoured
things in the eyewear, headwear and face cover slots, whatever their switch, then of the headwear and all fitted
to it, one at least as high with no face shield, or whose shield has no switch or is down; at the level's start
at once, at a change 0.6 s later and then at 4 a second (a change before that begins the wait anew); every batch's
alpha and size then times one less it (the level's factors 1; under night vision 1).

**The shaders** (one quad a flare at its light's place, facing the screen, added to the picture after the
transparent things, no depth test, both faces). `Custom/multiFlare` (the normal and flashlight flares): the
point moved 0.1 m toward the camera; none where the light's alpha is under 0.01 or the point is outside the view;
23 taps of the scene's depth around it, spread by the projection's scales times 0.003 over its depth, a tap
counting where the point is nearer than the scene there; the share counted (none, none drawn) shrinks it and
dims it; the screen's edge fades it over the outer eighteenth each way; past its near distance its size and alpha
go toward their far figures; its half size the projection's scales times its size times its distance from the
near plane times the share and the size's change, so a fixed angle on the screen, larger zoomed; its colour the
flare's times the alpha's change, the edge, the light's alpha again and the share; the picture times that times
0.6. `Custom/ShitOnScreen` (the noise flare): the point moved 1 m toward the camera (so none for a light under a
metre off); its taps and its distance taken as compiled, two of its tests not turned for the reversed depth, so
nearly every tap counts and its distance is the near plane's (a size in the world, about 3 cm a unit of its size
at a metre); a silhouette: from a point 0.4 m toward the camera, probes 1.25 cm apart, seven each way along the
view's x and y, each way the nearest clear one's eighth (1 where none is clear), the two ways multiplied where
the scene hides the point itself, one less that its share; a fine grain (the level's "Noise", 2 by 2 across the
screen, moved every frame) times 5 over its picture.

**Built** (2026-11-08: a flashlight shone on the camera blinds it, and for the light to come from, the dummies'
guns get a light): `trp_data::tactical` (`DeviceFlare`, `Flare`,
`FlareController`, `flare_pictures`: the atlas "Atlas2", `sharedassets3.assets` #11, and "Noise",
`resources.assets` #1379, read by number and checked by name), `trp_sim::flare` (the alpha, each flare's
figures, the crowding, the protection; the atlas's ten rectangles and the two shaders' figures as the level's
materials set them), `trp_sim::inventory::blindness_protection`, the app's `flare_view` (each frame once
everything is placed, on the main camera and the optic's) and `flare.wgsl` in `muzzle_view`'s pass, under
`TACTICAL DEVICES` with the dummies. The light: the dummies' AKM is the stock build with a Zenit B-10M handguard
and the Klesch-2U in its `mod_tactical` (`dummy::build`, in hand and in their kit), its model, its spot light and
its flares shown by `tactical_view::show_dummy_devices` in its first mode while the debug panel's `LIGHTS` is on
(`--dummy-lights` from the start). `PROVISIONAL`: each light's place and way as the frame leaves them (the game
moves the alpha in its `Update`, before the frame's animation); drawn after Bevy's tonemapping on the frame
encoded for the camera's effects, as the muzzle flash is, where the game adds them before its bloom and
tonemapping; the depth taps held to the screen's edge (the engine's sampler not read); the taps' pattern with
the screen's y down. **Seen** (`local\shots\flare_dummy_s229.png`: 3.5 m before the standing dummy, the light a
hot point in a wide bluish glare; `flare_side_s229.png`: from 3 m to its side, none). **Tested** (the flare
module's four: the alpha by the facing, sinking to −4 and two seconds back; a flare's figures; the crowding at
0.7 and further; the protection's wait and pace; `the_flares_protection_is_what_the_head_wears` with glasses
and the Altyn's shield up and down). **Not compared with the game**: no recording has a bot's light.

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
1), the rule to the last figure. **Ours** on the simulation's total ergonomics, which lacks the
gear's ergonomics penalty: the M60E6 build's 5 against the game's 4.75 (0.2875 for 0.288125). **A firearm
drawn** after other hands works its own out as its controller is made since 0.223.2 (`Hands::firearm_made`,
the host passing whether a combo unit on that weapon is on; before, it kept the last hands' figure unless
the combo state differed): weapon-handling spec, "The hip penalty, each weapon's own".

## State

On the item (`upd.Light`), no battery and no timer. In Press mode only the keys change it: it stays through
weapon switches, the inventory and saving, and a weapon drawn comes out with its device on; by the code a
slung weapon's light stays on ("Seen from outside, and put away", above; to be seen in a recording). In Hold
mode, off at every event above.

## Not read, or not settled

How T, Ctrl+T and T+B are told apart by the input (our key manager takes the longest combination; whether
the game's T alone also fires with Ctrl held is not read); the clips' motion (`tp_helmetrail`); the order
of the IR lights' switch against the recorder's frame, and of the lasers against the head light's placement
(a helmet laser may lag a frame); the HUD panel's own fade (ours the optic panel's: 0.5 s in, 3 s, 0.5 s
out); which of the two spots of the X400, the Klesch-2P and the DBAL is which mode's (the WF-501B's:
recorded, above); when within a reload its change of magazine works the hip penalty out again
(`WeaponModified`, which does, called by `InsertMagOperation`, `PullOutMagOperation` and
`ReloadExternalMagOperation`: read 2026-11-08; the app's build changed mid-reload keeps the old build's
figure, PROVISIONAL); the one frame at nothing of a light first lit; the flare's and the shaft's figures not
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
  unit on; a firearm drawn works its own out (`Hands::firearm_made`).
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
- **Since 0.167.0** the keys, Hold mode's let-go, the head light's coroutine and the hip penalty are the
  simulation's (`trp_sim::tactical::Devices`, one frame at a time, `Keys` and `Around` in, `Event`s out),
  run by the app (`tactical_view::tactical_keys`) and by the replay alike; the hip penalty worked out
  again whenever a request changes a combo unit's on, a let-go's too with the keys unheard; Hold mode let
  go as a change of the hands begins (`HandsHeld::changes`). The lasers' figures from a hit are the
  simulation's too (`tactical::laser_figures`, `laser_light_at`); the head's lights and lasers are placed
  by the camera as the frame before left it (`Tactical::head_camera`).
- **Since 0.218.0**: seen from outside, the head's lights and lasers on their objects on the body's model
  of what is worn (`body_view::HeadModels::device_on_body`: a fitted part's tree is hung in its prefab's
  order, so its object n is the hung tree's root plus n), and the head devices' `mode_*` objects there as
  their state shows; one's own flares gathered with the dummies' (`flare_view`: the weapon in hand's, the
  weapons hung on the body's, the head's from outside; in first person the head's are not gathered, since
  they face away from the eyes); the weapons hung on the body (`slung_view`, under `WEAPONS ON THE BODY`)
  with their devices as their items have them (`show_slung_devices`: the `mode_*` objects, the lights,
  the lasers; their figures read with the model, `icons::Model::devices`). **Seen**: the head light's
  pool ahead from the helmet and its flare from the front, the gun light's flare from the front, the
  slung AKM's light on the floor behind the character, the AKM drawn again still lit (`workbench.md`,
  0.218.0). **Not compared**: no recording has one's own light from outside nor a slung weapon's light.
- **Not built**: the light shaft, left out (by choice: the recorded
  game has the graphics option off); the one frame at
  nothing of a light first lit. (The flare is built: "The light shaft and the flare", 0.214.0.)

## Compared with

`courses6` (recorder 0.13.0, 2026-10-24), course 7's six scripts with the M60E6 FDE (a Klesch-2P and an
X400, a WF-501B on the helmet) and with the M4A1 (an Olight Baldr Pro; its `05-refusals` from 2.8 s and its
`06-modes-off` with the SPEAR, the script having drawn the first primary), by `cargo run -p trp-sim --bin
tactical -- <folder>` (`local\compare_c7_m60_s128.txt`, `compare_c7_m4_s128.txt`):

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

`courses8`'s group 3 (recorder 0.14.0, by hand): `2017-tactical-lights` (the M4A1 with an X400
and a PEQ-15, Ctrl+T through their modes, a canted sight), `rifle-with-only-one-light` (the SKS's XC1, one
mode), `pistol-tactical-light` (the USP's X400, twelve presses in 2.1 s), `tactical-laser-dark`,
`tactical-hold` (Hold mode, the inventory, a weapon switch, T+B), `headlamp` (the WF-501B: H and Ctrl+H
standing, prone, in blind fire, leaning, with presses in its lock) and `headlamp-sprint-reload` (refused in
the sprint, taken in a reload without the gesture). Their keys through our model of the input manager
(`byhand --scenario`), replayed on the recorded frames (`local\c8_tac_run_s169.sh`; the regression's
`r25` to `r31`, `tactical-hold` to 5.70 s), and `bin/tactical`:
- **Every frame of all seven**: each device's on and mode, the hands' `MOD SWITCH` (every press at rest,
  the pistol's quick ones, a one-mode device's Ctrl+T), every switch's sound (the hands' and the player's),
  the head light's lock and the gesture's flag (presses in the lock and in the sprint refused, the one in
  a reload taken without the gesture), the hip penalty and its ergonomics: the game's.
- **The weapon's devices**: all 27 states seen, each light lit with all its figures (the PEQ-15's
  indicator too), the models' `mode_*` objects and the lasers drawn, the game's; 13,173 lasers' beam,
  dot and dot's light by our figures from the recorded hit, to the fifth decimal, the light's place within
  0.05 mm.
- **The head light**: its way within 0.028 degrees by the camera as the frame before left it (2.6 by the
  frame's own), its place 5.3 and 4.7 mm rms in the two that walk, prone and sprint.
- **Not compared**: the weapon's lights' and lasers' own place and way frame by frame; the let-go by the
  inventory and the weapon switch (the replay plays neither; seen in the recording's timeline, the rule
  read from code); the T+B panel; Hold mode's own sound's place.
