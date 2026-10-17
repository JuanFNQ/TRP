# The pictures by their levels: texture quality and mip streaming

How much of each picture the game keeps on the graphics card. A picture comes with its levels of detail (mipmaps: the picture, then at half the size, a quarter, down to a pixel); the game either leaves its largest levels off every picture, by its texture quality, or streams them, keeping of each picture the levels its place on the screen needs. The drawing alone: nothing of the simulation changes.

Read from the decompiled game (`GraphicsSettingsController`, `GraphicsSettingsGroup`, `CameraManager`, `LevelSettings`, `ItemIconCreator`, `IconCreatorBase`), the install (`Graphics.ini`; the textures' own settings) and Unity's documented behaviour. In this work's own words.

## What the game does (read 2026-10-15)

**The settings.** `Graphics.ini` holds `TextureQuality` (0 low, 1 medium, 2 high), `MipStreaming` (on or off), `MipStreamingBufferSize` (16 to 64 MB, 32 by default) and `MipStreamingIOCount`. The install read on 2026-10-15 has quality 1 and streaming off.

**With streaming off** (`ChangeTexturesQuality`): Unity's `globalTextureMipmapLimit` is `TextureQualityToMipBias(quality)`, which is `clamp(2 - quality, 0, 2)` (one more in the game's "SD mode", which this work does not have). Every texture loses that many of its largest levels: none at high quality, the full size at medium, two levels at low.

**With streaming on**: the limit is 0, and `RefreshStreamingMemoryBudget` sets Unity's streaming:
- a budget of 1,024 MB at every quality (`1024f * Vector4.one`); a map sets its own on loading (`LevelSettings.ApplySettings`: its `MaxMapTextureMemory`, 1,024 at every quality unless the map says otherwise);
- a texture brought down at most `4 - quality` levels (`streamingMipmapsMaxLevelReduction`);
- every camera counted (`streamingMipmapsAddAllCameras`), and the levels nothing needs given up (`streamingTextureDiscardUnusedMips`);
- each camera's streaming bias is `TextureQualityToMipBias(quality)` levels (`CameraManager`; 1.5 fewer with DLSS or FSR, which this work does not have);
- `MipStreamingBufferSize` is Unity's `asyncUploadBufferSize`, the buffer the levels go up to the card through.

**Which textures are streamed.** Unity streams only a texture imported for it (`m_StreamingMipmaps`), and draws the rest whole while streaming is on. Every one of the AKM's 27 textures is (priority 0; `trp-data/tests/install.rs`, `the_weapon_s_textures_are_marked_for_streaming`). Nine of the range's other pictures are not (the app counts them).

**How Unity picks a texture's level** (Unity's manual on mipmap streaming; the engine's own sum is not published): from each renderer that draws with it, its bounds' distance from each camera, the camera's field of view and height in pixels, and the mesh's texel density (`Mesh.GetUVDistributionMetric`: its surface over its pictures' surface), with the camera's bias; within the budget, what is farthest is brought down first; the levels are read from the texture's file and sent to the card away from the frame.

**The inventory's pictures** are drawn from every texture whole: `ItemIconCreator` sets `requestedMipmapLevel` 0 on each, and the budget is raised while it draws (`IncreaseMemoryBudgetForIcon`).

## What is done here

**The settings** are the options' `TEXTURE QUALITY` (LOW, MEDIUM, HIGH) and `MIP STREAMING` (ON, OFF) on the graphics page, each taken at the next start (`RESTART REQUIRED`); `--texture-quality <0-2>` and `--mip-streaming on|off` for one run. Until changed: the install's own quality, and streaming on (the user, 2026-10-15: "can we enable mip streaming?"; the install's own game has it off).

**With streaming off**, every picture of a model (the range's, the body's, the rigs', the things on the ground, the outfits', a weapon of one's own's) loses `clamp(2 - quality, 0, 2)` of its largest levels as it is added to the engine (`streaming::add`), never below the last level of whole blocks of a compressed picture.

**With streaming on**, a picture the game streams goes to the card from the most it may be brought down, and every tenth of a second (`ESTIMATE`) the level it wants is worked out:
- for each mesh shown (in sight of a camera or not: Unity's is said to go by distance alone, `PROVISIONAL`) and drawing with it, for each perspective camera drawing: `level = ½·log2(texels · d² · tan²(fov/2) / (density · (H/2)²)) + bias`, where `texels` is the picture's width by its height, `d` the distance from the camera to the mesh's bounds (none inside them), `fov` the camera's vertical field of view, `H` its height in pixels, `density` the mesh's surface over its pictures' surface at its size in the world (`PROVISIONAL`: Unity's sum is not published), and `bias` the game's `2 - quality`;
- the least over every mesh and camera, to the whole level below, never less than 0 nor more than `4 - quality` (or the last level of whole blocks); a picture nothing shown draws with goes down to that most;
- within 1,024 MB, the farthest given up a level at a time;
- a picture raised is raised at once, the nearest first, up to 32 MB a decision (`ESTIMATE`, after the game's 32 MB buffer, which in Unity is a ring the uploads pass through, not a limit); one wanted lower waits 2 s first (`ESTIMATE`), so that a thing passing in and out of view does not send its pictures up again and again.

**How a level changes.** A picture on the card is never changed in place (changing the engine's pictures as they came drew the whole range black: `findings.md`). A new copy from the level wanted is made away from the frame, from the texture's bytes read again from its bundle's file (only the compressed blocks they lie in, as Unity reads a streamed texture's levels; `trp_data::unity::ResourcePlace`, `Bundle::read_range`), made as it was made the first time; the materials drawing with the picture are pointed at it, and the copy before goes. The first copy, at the most the picture may be brought down, is kept as long as the picture (a rig waits for it to be on the card; going back down to it is at once). A picture with no bundle (a weapon of one's own's, the other game's suits, or one whose levels are inside its serialized file) is kept whole in memory instead (25 MB of 1,943 in the measure below).

**The inventory's pictures** are drawn with every picture of the item's model whole (`PreparedLooks::add_with_pictures`), as the game's.

## Built (0.14.0)

- `trp-app/src/streaming.rs`: the settings, `add`, the levels' layout of a picture, `take_new` (pictures, meshes' texel densities and materials taken in; a material gone, let go of), `stream` (the levels wanted, the budget, the copies made and taken in). Tests: the settings' figures; a compressed picture cut at whole blocks.
- `trp-data`: `ResourcePlace` (where a resource's bytes are in its bundle's file) and `Bundle::read_range`; `read_texture_shared` (a texture and its place); `TextureData::streaming_mipmaps`. Tests: a texture read again is the same bytes; the AKM's textures are streamed.
- `render_data.rs`: every model's picture through `streaming::add` with its source; the options' two rows; `TRP_GPU_MEMORY` (`render_timing::add_memory`): the graphics allocator's own figures and the pictures the renderer holds, every two seconds.

## Compared with what

**Against the game: nothing.** No recording says which level the game holds of a texture; the formula above is Unity's documented inputs put together by this work.

**Captures** (`--tap 30:f1:2`, frame 500, the rifle in hand at the range; `local\picdiff_s60.ps1`, the mean difference of the three channels summed, 0 to 765), each against high quality with streaming off (the app as it was):

| Run | The whole view | The rifle and hands | The tables |
|---|---|---|---|
| high, streaming on | 0.011 | 0.064 | 0.023 |
| medium, streaming off | | 0.843 | 0.042 |
| medium, streaming on | | 0.079 | 0.037 |
| low, streaming on | | 0.708 | 0.342 |

**Memory**, 48 rifles thrown out of their case one by one (43 thrown; `local\peak_memory_s60.ps1`, the app's own figures with `TRP_GPU_MEMORY` at the end, Windows' at their most):

| Quality, streaming | Pictures on the card | Allocated for the card in all | Windows: dedicated at most | Private memory at most |
|---|---|---|---|---|
| high, off (as before) | 1,970 MB | 2,477 MB | 2,633 MB | 4,866 MB |
| high, on | 232 MB | 740 MB | 1,353 MB | 3,071 MB |
| medium, off | 512 MB | 1,020 MB | 1,353 MB | 3,958 MB |
| medium, on | 135 MB | 651 MB | 1,097 MB | 3,676 MB |

(Windows' dedicated figure counts the allocator's reserved blocks, which are not given back as they empty.) Kept whole in memory instead of read again from the bundles, the same run with streaming on took 2.6 GB more private memory than with it off; holding the bundles open took about as much.

**The frame**: walking with the rifle and opening the inventory (`--equip --hold w --tap 500:tab`, frames 200 to 909): 164.8 frames a second by the mean at both qualities either way, the spread 0.55 to 0.59 ms.

## PROVISIONAL, ESTIMATE

- `PROVISIONAL`: the formula of the level; the texel density as the whole surface over the whole of the pictures; a mesh counted while shown, in sight or not.
- `ESTIMATE`: a decision every 0.1 s; 32 MB raised a decision; 2 s before a picture is brought down.

## Not built

- A map's own budget (no map here); the SD mode; the bias under DLSS or FSR; the textures' priorities (the AKM's are all 0); `MipStreamingIOCount`.
