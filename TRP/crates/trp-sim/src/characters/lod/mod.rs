//! Which level of detail a camera draws of a group, by Unity's rule and by TRP's own.
//! After the third-person spec, "Levels of detail". The level as Unity's
//! engine picks it (`LODGroupManager::CalculateLODMasks` and
//! `CalculateLODMask`, read in `UnityPlayer.dll` with Ghidra),
//! kept as the game's for reference and its test against
//! recordings of the game; and **TRP's own rule**, by which the app draws
//! (below). Pure arithmetic; the groups are `trp_data::lod`'s
//! and the cameras the host's.
//!
//! The engine reckons a distance `d × 2 tan(fov / 2) / bias` (`d` from the
//! camera's place to the group's reference point in the world, `fov` the
//! camera's vertical field of view, `bias` Unity's `QualitySettings.lodBias`
//! as it stands while that camera culls) and draws the first level whose
//! `size / height` is still beyond it (`size` the group's times its object's
//! largest scale). Past the last, nothing. That is the documented rule: a
//! level is drawn while the group fills more of the view's height than the
//! level's least share, `relative_height`.

use std::f32::consts::PI;

/// The share of the view's height a group fills (Unity's documented
/// "screen relative height"): `size × scale × bias / (2 d tan(fov / 2))`.
/// Infinite at the camera's own place.
pub fn relative_height(size: f32, scale: f32, distance: f32, vfov_deg: f32, bias: f32) -> f32 {
    size * scale * bias / (2.0 * distance * half_tan(vfov_deg))
}

/// The level drawn at a share of the view's height: the first whose least
/// share it is above. None: nothing is drawn.
pub fn level(heights: &[f32], relative: f32) -> Option<usize> {
    heights.iter().position(|&height| relative > height)
}

/// The level a camera draws, reckoned as the engine reckons it (its
/// distance against each level's `size / height`; the same as
/// `level(heights, relative_height(..))` but for the last bit of rounding):
/// the first level whose reach the camera is within, none past the last's.
/// Exact on three recordings of the game (`the_games_pick_as_the_three_
/// recordings_drew_it`). A cross-fading group (fade mode 1; 18 worn
/// things' and 4 weapons' groups) is switched at once here, where the
/// engine draws both levels across a band before the switch where the
/// quality allows it: not modelled, as no group of the recordings fades
/// and the app draws by TRP's own rule (below).
pub fn pick(size: f32, scale: f32, distance: f32, vfov_deg: f32, bias: f32, heights: &[f32]) -> Option<usize> {
    let world = size * scale;
    // (The engine's own form: tan((fov/360 + fov/360) × π × 0.5) × 2 / bias.)
    let reckoned = distance * (2.0 * half_tan(vfov_deg)) / bias;
    let farthest = heights.iter().map(|&height| world / height).fold(0.0f32, f32::max);
    if heights.is_empty() || reckoned >= farthest {
        return None;
    }
    heights.iter().position(|&height| reckoned < world / height)
}

/// The distance a level stops being drawn at (its least share reached):
/// `size × scale × bias / (2 height tan(fov / 2))`.
pub fn reach(size: f32, scale: f32, height: f32, vfov_deg: f32, bias: f32) -> f32 {
    size * scale * bias / (2.0 * height * half_tan(vfov_deg))
}

/// The bias a camera's controller culls with (`CameraLodBiasController`:
/// the bias times `tan(fov / 2) / tan(max_fov / 2)`, set by the camera
/// manager's `Fov` and put on in `OnPreCull`): on the main camera, whose
/// `max_fov` is the options' field of view, so that a narrower view while
/// aiming picks as the options' view does.
pub fn controlled_bias(bias: f32, fov_deg: f32, max_fov_deg: f32) -> f32 {
    bias * half_tan(fov_deg) / half_tan(max_fov_deg)
}

fn half_tan(fov_deg: f32) -> f32 {
    (fov_deg / 360.0 * PI).tan()
}

// ---------------------------------------------------- TRP's own rule
//
// (Third-person spec, "TRP's own rule": the
// levels of detail and the culling are TRP's own, an exception to rule
// 8, a pass of TRP's own, for speed, as the game's own is slow.) The game's rule above is kept as the game's, for reference
// and its test against the recordings; the app draws by this one.
//
// A level may be drawn wherever the change it makes cannot be seen: where
// its deviation from the fullest (`trp_data::lod::deviations`, the
// largest distance between the two surfaces, both ways, metres) comes to
// under `tolerance` pixels in the view that draws it, at that view's own
// resolution and field of view. So a level is had by the size of a pixel
// at the piece, in metres (`pixel`): level k is allowed from a pixel of
// `deviation[k] / tolerance` on (`switches`), the coarsest allowed drawn.
// Nothing is culled alone: past its last level a group keeps its coarsest,
// and a whole thing (a character, a loose thing, a prop) goes only when
// all of it covers under `CULL_PIXELS` (`culled_whole`). Across a switch
// the two levels are cross-faded by a dither over `FADE` of the pixel's
// size beyond the switch (`dither`, as Bevy's dithered visibility ranges
// count it).

/// How much of a pixel the fade between two levels takes: the coarser
/// level fades in, and the finer out, while the pixel at the piece grows
/// from the switch to `1 + FADE` times it (a share of the distance, at a
/// given view). `ESTIMATE`, set by eye from captures: about a step of a
/// metre at 7 m, short enough to be gone while walking past.
pub const FADE: f32 = 0.15;

/// How much sooner, as a share of the pixel at its switch, a coarser level
/// begins to fade in than the finer one begins to fade out, so that the two
/// cover every pixel between them though the shader fades each piece by its
/// own origin (Bevy hands it the mesh's place): a rigid model's levels on
/// objects of their own stand apart (the AKM's body's two 0.30 m, a
/// headset's 0.11 m, in the app's log), and a share of the distance that
/// far apart would leave pixels undrawn without it. Meanwhile some pixels
/// are drawn by both, the nearer showing. `ESTIMATE`: 5%, a third of the
/// fade (0.30 m at 6 m).
pub const OVERLAP: f32 = 0.05;

/// The size on screen below which a whole thing is not drawn, pixels.
/// `ESTIMATE` (a pixel or two): a thing under a pixel
/// is at most one pixel's shade.
pub const CULL_PIXELS: f32 = 1.0;

/// The pixels of deviation allowed, from the options' `OBJECT LOD QUALITY`
/// (the game's bias, 2 to 4 by halves): `2 / bias`, so one pixel at the
/// game's own 2 (the install's, and the default) and half a pixel at 4.
/// Every switch distance scales with the setting as the game's bias scales
/// the game's (a bias of 4 keeps each level twice as far).
pub fn tolerance(bias: f32) -> f32 {
    2.0 / bias.max(0.5)
}

/// The size of a pixel at a distance, metres, in a view `pixels` high whose
/// projection's [1][1] is `clip_y` (`1 / tan(vfov / 2)` for a perspective
/// view; `2 / height` for an orthographic one, whose pixel is the same at
/// any distance: a shadow map's texel).
pub fn pixel(distance: f32, pixels: f32, clip_y: f32, orthographic: bool) -> f32 {
    2.0 * if orthographic { 1.0 } else { distance } / (pixels * clip_y).max(1e-9)
}

/// The pixel size from which each level is allowed (its deviation, times
/// its object's world scale, over the tolerance), the fullest's 0, never
/// less than a finer level's. An unmeasured group (no deviation) allows
/// its fullest alone: every coarser level's is infinite.
pub fn switches(deviation: &[f32], levels: usize, scale: f32, tolerance: f32) -> Vec<f32> {
    let mut out = Vec::with_capacity(levels);
    for level in 0..levels {
        let allowed = if level == 0 {
            0.0
        } else {
            deviation.get(level).map_or(f32::INFINITY, |&d| d * scale / tolerance.max(1e-6))
        };
        out.push(allowed.max(*out.last().unwrap_or(&0.0)));
    }
    out
}

/// The level drawn steadily at a pixel size: the coarsest allowed. Never
/// none: past its last level a group keeps its coarsest.
pub fn steady_level(switches: &[f32], pixel: f32) -> usize {
    switches.iter().rposition(|&from| pixel >= from).unwrap_or(0)
}

/// The pixel sizes a piece drawn at `levels` (a bit each; the first level
/// the lowest bit) is drawn between: from its finest level's switch to its
/// coarsest's next (infinite past the last level). A piece at no level of
/// the group is never drawn (both infinite).
pub fn band(switches: &[f32], levels: u8) -> (f32, f32) {
    let listed = |level: usize| level < 8 && levels & (1 << level) != 0;
    let (Some(first), Some(last)) = ((0..switches.len()).find(|&l| listed(l)), (0..switches.len()).rfind(|&l| listed(l))) else {
        return (f32::INFINITY, f32::INFINITY);
    };
    (switches[first], switches.get(last + 1).copied().unwrap_or(f32::INFINITY))
}

/// A piece's dither at a pixel size, as Bevy's dithered visibility ranges
/// count it (`visibility_range_dither`): 0 drawn whole, -16 and 16 not at
/// all; from -16 up to 0 fading in over `FADE` from `OVERLAP` before its
/// band's start, from 0 up to 16 fading out over the `FADE` past its
/// band's end. Two pieces of neighbouring levels at the same pixel size
/// take complementary pixels and some more. `trp_lod_dither` in the app's
/// patch of Bevy's mesh functions is the same in the shader.
pub fn dither(band: (f32, f32), pixel: f32) -> i32 {
    let (from, to) = (band.0 * (1.0 - OVERLAP), band.1);
    if pixel < from {
        return -16;
    }
    if from > 0.0 && pixel < from * (1.0 + FADE) {
        return -16 + ((pixel - from) / (from * FADE) * 16.0).round() as i32;
    }
    if pixel < to {
        return 0;
    }
    if pixel < to * (1.0 + FADE) {
        return ((pixel - to) / (to * FADE) * 16.0).round() as i32;
    }
    16
}

/// Whether a piece is drawn at all at a pixel size (any dither short of
/// ±16), with a margin for the shader's own rounding of the same figures.
pub fn drawn(band: (f32, f32), pixel: f32) -> bool {
    pixel >= band.0 * (1.0 - OVERLAP) * (1.0 - 1e-4) && pixel < band.1 * (1.0 + FADE) * (1.0 + 1e-4)
}

/// Whether a whole thing of `size` metres (its largest group's world size)
/// goes, at a pixel size: all of it under `CULL_PIXELS`.
pub fn culled_whole(size: f32, pixel: f32) -> bool {
    size < CULL_PIXELS * pixel
}

/// A thing's size on screen, pixels: `size` metres (a whole thing's largest
/// group's world size) over the view's pixel at `distance` (`pixel`), in a
/// view `pixels` high whose projection's [1][1] is `clip_y`. 14d's rule
/// (a whole thing goes under `CULL_PIXELS`, `culled_whole`), which the
/// app's posing of characters goes by too (`posing_drawn`).
pub fn pixels_tall(size: f32, distance: f32, pixels: f32, clip_y: f32, orthographic: bool) -> f32 {
    size / pixel(distance, pixels, clip_y, orthographic).max(1e-12)
}

/// The size on screen from which a character's every frame is posed and
/// drawn between frames, at a tolerance of one pixel (`OBJECT LOD
/// QUALITY` at the game's 2), pixels. `ESTIMATE`, this work's (third-person
/// spec, "Posing by distance and off screen: the game's, and TRP's own"):
/// the hands' 10 cm move is a pixel at about 17 px on a figure of 1.75 m,
/// so twice that.
pub const POSED_EVERY_FRAME_PIXELS: f32 = 32.0;

/// Below `POSED_EVERY_FRAME_PIXELS`, a character's newest frame is posed and
/// drawn every this many of its frames and held between, at a tolerance of
/// one pixel. `ESTIMATE`, this work's: at the 120 frames a second of the
/// simulation, 30 poses a second, its place moved on every frame.
pub const HELD_FOR_FRAMES: u32 = 4;

/// How a character is drawn by its size on screen (`posing_drawn`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PosingDrawn {
    /// Every frame posed, drawn between two of its frames.
    EveryFrame,
    /// Its newest frame posed every so many of its frames and held between.
    Held(u32),
    /// Nothing posed for drawing: culled whole, or out of every view.
    Not,
}

/// How a character is drawn by its largest size on screen over the views
/// (`pixels_tall`; none where no view sees it), at the tolerance of the
/// options' `OBJECT LOD QUALITY` (`tolerance`): both of the figures above
/// scale with it as 14d's switches do (at a bias of 4, half a pixel: every
/// frame from 16 px, held for 2 frames below it).
pub fn posing_drawn(pixels_tall: Option<f32>, tolerance: f32) -> PosingDrawn {
    match pixels_tall {
        Some(tall) if tall >= POSED_EVERY_FRAME_PIXELS * tolerance => PosingDrawn::EveryFrame,
        Some(tall) if tall >= CULL_PIXELS => PosingDrawn::Held(((HELD_FOR_FRAMES as f32 * tolerance).round() as u32).max(1)),
        _ => PosingDrawn::Not,
    }
}

/// A model's groups switching together (a weapon's parts with their
/// weapon; a thing on the ground with its parts): each group's deviation at
/// a level becomes the largest of every measured group's at that level, a
/// group of fewer levels taken at its last. An unmeasured group (empty) is
/// left out of the largest and stays unmeasured (its fullest at any
/// distance).
pub fn together(groups: &[Vec<f32>]) -> Vec<Vec<f32>> {
    let at = |level: usize| groups.iter().filter(|d| !d.is_empty()).map(|d| d[level.min(d.len() - 1)]).fold(0.0f32, f32::max);
    groups.iter().map(|d| if d.is_empty() { Vec::new() } else { (0..d.len()).map(|level| if level == 0 { 0.0 } else { at(level) }).collect() }).collect()
}

/// The largest half float: a band's end that is never reached (the
/// shader is handed no infinity).
pub const NEVER: f32 = 65504.0;

/// The bits of a mesh's tag (Bevy's `MeshTag`) that hold its band; the top
/// two are the decals' (the game's stencil type of its material: the app's
/// `decal_view`), which each writer keeps as it finds them.
pub const BAND_BITS: u32 = 0x3fff_ffff;

/// A band as the shader is handed it, in a mesh's tag's low 30 bits: its
/// two ends as half floats without their sign (never negative), the start
/// in the low 15 bits, an end past `NEVER` taken as `NEVER`. Never 0, the
/// band of a mesh with none.
pub fn tag(band: (f32, f32)) -> u32 {
    let bits = (u32::from(half(band.0.max(0.0).min(NEVER))) & 0x7fff) | ((u32::from(half(band.1.max(0.0).min(NEVER))) & 0x7fff) << 15);
    if bits == 0 {
        // (0, 0): drawn nowhere past its own place; as (0, the least half).
        1 << 15
    } else {
        bits
    }
}

/// A tag's band as the shader reads it (each 15 bits a half float with its
/// sign 0, by `unpack2x16float`): the figures the app's pick must go by too.
/// The decals' bits are not read.
pub fn untag(tag: u32) -> (f32, f32) {
    (unhalf((tag & 0x7fff) as u16), unhalf(((tag >> 15) & 0x7fff) as u16))
}

/// A float as a half float (IEEE 754 binary16), rounded to the nearest,
/// ties to even; too large, infinite.
fn half(v: f32) -> u16 {
    let bits = v.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exponent = ((bits >> 23) & 0xff) as i32;
    let mantissa = bits & 0x7f_ffff;
    if exponent == 0xff {
        return sign | 0x7c00 | if mantissa != 0 { 0x200 } else { 0 };
    }
    let e = exponent - 127 + 15;
    if e >= 0x1f {
        return sign | 0x7c00;
    }
    if e <= 0 {
        if e < -10 {
            return sign;
        }
        let m = mantissa | 0x80_0000;
        let shift = (14 - e) as u32;
        let kept = m >> shift;
        let rest = m & ((1 << shift) - 1);
        let halfway = 1 << (shift - 1);
        let rounded = if rest > halfway || (rest == halfway && kept & 1 == 1) { kept + 1 } else { kept };
        return sign | rounded as u16;
    }
    let kept = mantissa >> 13;
    let rest = mantissa & 0x1fff;
    let mut out = ((e as u32) << 10) | kept;
    if rest > 0x1000 || (rest == 0x1000 && kept & 1 == 1) {
        out += 1;
    }
    sign | out as u16
}

fn unhalf(h: u16) -> f32 {
    let sign = u32::from(h & 0x8000) << 16;
    let exponent = u32::from((h >> 10) & 0x1f);
    let mantissa = u32::from(h & 0x3ff);
    let bits = match exponent {
        0 if mantissa == 0 => sign,
        0 => {
            let (mut e, mut m) = (127 - 15 + 1, mantissa);
            while m & 0x400 == 0 {
                m <<= 1;
                e -= 1;
            }
            sign | (e << 23) | ((m & 0x3ff) << 13)
        }
        0x1f => sign | 0x7f80_0000 | (mantissa << 13),
        _ => sign | ((exponent + 127 - 15) << 23) | (mantissa << 13),
    };
    f32::from_bits(bits)
}

#[cfg(test)]
mod tests;
