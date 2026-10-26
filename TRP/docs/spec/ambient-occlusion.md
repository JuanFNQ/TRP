# Ambient occlusion: the game's SSAO setting

How the game darkens creases, corners and the ground under things (its graphics setting `SSAO`), and how the app draws it. Written 2026-10-19 after the user's decision (asked whether DLSS, FSR and SSAO could be added: "Can we just do ssao and then just do performance passes?"; DLSS and FSR were left out, as an upscaler belongs to whichever program draws the final frame, which in a passthrough is the host: `decisions.md`).

**Where the knowledge comes from.** The rules are the game's code (`CameraManager.SetSSAO`, `HBAO`, `HBAO_Core`, `SSAOMask`, `PostEffects.ContactShadows`) and its compiled shader `Hidden/HBAO` in the install, disassembled with the Windows SDK's `fxc /dumpbin` (`probe shaders "Hidden/HBAO" <folder>`, whose programs lie in the blob's second segment). The horizon-based method itself is NVIDIA's published one (Bavoil, Sainz and Dimitrov, 2008). **Compared with** recorder 0.11.0's scene of Customs (`local\traces\courses3\0-light-outside`): the camera's `HBAO` component as the game ran it, at the user's setting. The picture is not compared (no capture of the game's screen).

## What the setting turns on

The setting `Settings/Graphics/Ssao` (the user's `Graphics.ini`: `FastestPerformance`) has six choices: `Off`, `FastestPerformance`, `FastPerformance`, `HighQuality`, `HighestQuality`, `ColoredHighestQuality`. `SetSSAO` turns the camera's `HBAO` on for every choice but `Off` and applies the preset of the same name. From `HighQuality` up it also turns on a second effect, Unity's cinematic `AmbientOcclusion` at its low sample count (in the recording, off; its figures there: intensity 3.32, radius 0.02, cut off at 10 m, a field of BSG's own; downsampled; over the whole picture, not the ambient alone).

Two other things on the camera are not part of it:

- `SSAOMask` draws a mask (`_SSAOMask`) that only `Hidden/FakeCharacterGI` reads.
- `ContactShadows` (Keijiro Takahashi's published effect) is off in the running game, with no light and no samples (the recording). **The game draws no contact shadows.**

## The presets

A preset first puts every setting back to its default, then changes a few (`HBAO_Core.ApplyPreset`). The defaults: quality Medium, no deinterleaving, full resolution, the dithered noise, radius 0.8 m, at most 128 pixels, angle bias 0.05, intensity 0.75, no multi-bounce, no off-screen samples, AO faded out from 100 m and gone at 150 m (maximum distance 150, falloff 50), normals from the G-buffer, base colour black, no colour bleeding, the Medium blur at sharpness 8 at full size.

| Choice | Quality (directions × steps) | Radius | At most | Blur |
|---|---|---|---|---|
| `FastestPerformance` | Lowest (3 × 2) | 0.5 m | 64 px | ExtraWide, at half size |
| `FastPerformance` | Low (4 × 3) | 1 m | 64 px | Wide, at half size |
| `HighQuality` | High (8 × 4) | 1.46 m | 128 px | Narrow |
| `HighestQuality` | Highest (8 × 6) | 1.46 m | 256 px | Narrow |
| `ColoredHighestQuality` | Highest (8 × 6) | 1.46 m | 256 px | Narrow, with colour bleeding |

(Medium, 6 × 4, is the defaults' and no choice's.) **Compared**: the recorded component at `FastestPerformance` has exactly that row and the defaults beneath it.

## Where it runs

`HBAO` is an image effect on the opaque scene: it runs after everything opaque is drawn and lit, before the camera's `TOD_Scattering` (the next effect on the camera) and before anything transparent. It reads the camera's depth and the G-buffer's normals (world normals, normal maps included, stored as `n × 0.5 + 0.5` in ten bits each). Its textures are the engine's default kind: four channels of eight bits, filtered bilinearly, clamped at the edges. Each frame, at the camera's own size:

1. **The occlusion**, into a texture cleared to white;
2. **the blur across**, into a texture of half the size where the preset says so (else the same size);
3. **the blur down**, back into the first;
4. **the composite**: the picture multiplied by it.

The camera's far plane is 10,000 m (`SetOverallVisibility`; the recording has 10,000), and the occlusion keeps the depth as a share of it.

## 1. The occlusion

For a pixel at distance `z` along the view (its linear depth):

- **Beyond the maximum distance** (150 m) the pixel is left as cleared: no occlusion. The sky is so.
- Its place in the view is `((2u − 1) a z, (1 − 2v) b z, z)`, with `u` across from the left, `v` up from the bottom, `b` the tangent of half the vertical field of view and `a = b × width / height`: a view with x to the right, y **down**, z forward. Its normal is the G-buffer's, turned by the camera's world-to-camera matrix and then y and z turned round, into the same view.
- **The reach in pixels**: `R × height / (4 b z)`, at most the preset's pixels, where `R` is the radius in metres. It is split into steps of `reach / (steps + 1)`.
- **The noise**: a texture of 4 × 4, read at the pixel's place by whole texels, repeating: each texel holds a turn's cosine and sine and a jitter. Its numbers come from a fixed table of 32 in the game's code (`MersenneTwister.Numbers`), taken two at a time down each column, then across: the turn is `2π × first / directions`, the jitter the second. The texture is of bytes, so a negative cosine is kept as nought: a texel whose turn passes a quarter turn holds a shorter, steeper direction. Its rows run up from the bottom (Unity renders into a texture upside down on Direct3D so that its rows count up, as its manual says; the screen's pixels are counted so too).
- **The march**: for each direction (the noise's, then turned on by a whole turn shared out among the directions), and each step, the place `round(direction × (jitter × step + 1 + n × step))` pixels off (rounded to the nearest, a half to the even), the depth there read (an edge's if off the screen), its place in the view worked out as above from its own `u`, `v` and depth. With `V` from the pixel to it: `max(0, min(1, N·V / |V| − bias)) × max(0, min(1, 1 − |V|² / R²))`; a sample on the pixel itself adds nothing.
- **The sum**, times `2 / (1 − bias)`, over directions × steps: the occlusion `ao = max(0, min(1, 1 − sum × 2 / ((1 − bias) × directions × steps)))`, faded to 1 between 100 and 150 m.
- **Written**: red and green the depth as a share of the far plane in two bytes (`x − frac(255x)/255` and `frac(255x)` of the share's fraction), blue 1, alpha the occlusion.

## 2 and 3. The blur

Across, then down, each over the preset's reach of texels (Narrow 2, Medium 3, Wide 4, ExtraWide 5) on either side of the middle, in the destination's texels, read bilinearly from the source. Each tap's weight is `2^(−k² × 2 / reach² − (Δd × 10000 × sharpness)²)`, where `k` is its place and `Δd` the difference between its two-byte depth and the middle's; the middle weighs 1. The result is the weighted mean of the occlusion, with the middle's depth kept. At half size the first blur reads the full texture at its own half-size texels, and the second reads the half texture at full-size ones: both by bilinear filtering.

## 4. The composite

`f = min(ao ^ intensity, 1)`; the colour `base + f × (1 − base)` (with black, `f`) multiplies the picture's red, green and blue; its alpha stays.

## On the range (built 2026-10-19, 0.79.0)

Under the options' `AMBIENT OCCLUSION` (on unless switched; restart required), at the choice of the GRAPHICS page's `SSAO` (the game's six, in its own words; restart required): the install's own until changed (`Graphics.ini`'s `Ssao`, read, never written; the game's default, `Off`, where it says nothing), or `--ssao <choice>` for a run. `--ssao-display ao` shows the occlusion alone, as the game's display mode `AOOnly` does (its composite's colour, opaque). Off, or at `Off`, nothing of it is drawn and the camera has no normal prepass. The passes are `hbao_view.rs` and `hbao.wgsl`; the presets, the noise and the passes' figures `trp_sim::hbao`.

- **The normals** come from Bevy's normal prepass, the camera's opaque meshes drawn once more before the scene: world normals with their normal maps, in the same ten bits as the game's G-buffer. The first-person body, drawn as its shadow alone (`shadow_only.rs`), keeps nothing in that pass too, and still casts its shadow.
- **The depth** is the scene's own (`muzzle_view`'s copy, the first sample of each pixel), turned into the distance along the view through the camera's own projection.
- **Its place**: right after the depth's copy, in the game's numbers, before the sky's scattering and the effects (`scattering_view`, `explosion_view`), as the game's runs after its opaque scene and before its scattering. Everything Bevy draws is opaque. **The frame is in the game's numbers for it on every frame** (since 0.109.1, `gamma_ui`): until then the frame was encoded only while a camera effect showed, so with the lighting switches off the occlusion multiplied linear light at rest and the game's numbers while the inventory's blur or a shot was on, darker (the user: "when opening the inventory it gets darker").
- **Its textures** are the game's: eight bits a channel, the depth in two bytes, the blur bilinear.
- The app draws with 4x MSAA, which the game does not: the occlusion of a pixel's first sample darkens all of its samples.

No figure is the app's own. `PROVISIONAL`: the noise's bytes rounded to the nearest (the engine's rounding when a colour is set into a texture of bytes was not read; a byte either way turns a direction by at most a quarter of a degree).

**Measured again** (2026-10-23, 0.109.1, `--start lanes` at the install's `FastestPerformance`, 1,600 by 900; `local\ao_ratio_s120.ps1`): over every fourth pixel where the occlusion shown alone is below 240 of 255 (2,327) and below 225 (939), the brightness with it against without it is 0.759 and 0.662 (before the fix, at rest: 0.892 and 0.816; multiplying the game's numbers predicts their 2.2nd powers, 0.778 and 0.639); the view with the inventory open is the same as before the fix (a mean difference of 0.12 of 765). The figures of 2026-10-19 below were taken at rest, on the linear frame.

**Measured** (2026-10-19, at the user's `FastestPerformance`, 1,600 by 900, an RTX 3080 on Vulkan; captures `local\shots\ao_*_s102.png`): in the hall, the wall-floor joins and the cover block's base at 0.80 of their brightness without it, the open floor at 1.000; the glove and the handguard at 0.85 to 0.89, as the 0.5 m radius reaches across the whole rifle and hand at arm's length; on the surfaces course, the steps between the strips and the far boxes' feet; the sky, the horizon and the open ground untouched; no trace of the first-person body, and its whole shadow on the ground; on DirectX 12, one view looked at, the same as on Vulkan. **Its cost**: three rounds of the light run each way (`local\perf_ao_s102.ps1`), 3.94 ms a frame without it and 4.06 with it, the mean of the means (two rounds 0.19 ms apart, the third within the runs' spread). **Not compared**: the picture with the game's (no capture of the game's screen).

## Not built

The second effect of `HighQuality` and up (Unity's cinematic `AmbientOcclusion` with BSG's cut-off; its compiled shader not read): with those choices the app draws `HBAO` alone. Colour bleeding (`ColoredHighestQuality`): it reads the G-buffer's albedo and the lighting buffer, which the app does not have. Deinterleaving, multi-bounce and the off-screen samples' share, which no choice uses.
