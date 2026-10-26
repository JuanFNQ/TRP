// The face shield's mask over the first person's view (`visor_view.rs`;
// inventory spec, "The face shield's mask"), as the game's compiled
// `Hidden/VisorEffect` and `Hidden/FastBlur` do it: the scratches laid over
// the frame (pass 0), the glass's distortion toward the edges (pass 2), the
// frame blurred (the fast blur's three passes), and the blur and the frame's
// mask (pass 3). Pass 1, the cracks' refraction, has nothing to do: no hits
// are kept on a shield, so its crack picture is empty. The game's texture
// coordinates run from the lower left: a place is taken so, and its pictures
// are sampled there as stored; the frame is sampled at Bevy's (from the
// upper left). Every pass ends `C + inside * fade * (X - C)`.
// `PROVISIONAL`: the mask's pictures are sampled at their full size (level
// 0), where the engine picks a level by the pixels' footprint (the same at
// 1920 by 1080: 2048 texels over 0.9 of the width).

struct Visor {
    // `_SwingValue`, `_Intensity`, `_EdgeFade`, `_Rotation` (radians),
    // `_HitZoomValue`.
    swing: vec2<f32>,
    intensity: f32,
    edge_fade: f32,
    rotation: f32,
    hit_zoom: f32,
    // `_ScratcesIntensity`; the scratches' light (`lerp(_MinLightValue, 1,
    // _IsAffectedByLight)`); `_DistortIntensity`; `_BlindnessCoef`.
    scratches_intensity: f32,
    light: f32,
    distort_intensity: f32,
    blindness: f32,
    // 0 where every picture but the mask is cleared (a narrow mask,
    // `ClearAllTextures`: the shader's black); 0 where the mask is none
    // (the shader's white).
    textured: f32,
    framed: f32,
    // The fast blur's `_Parameter.x` for each of its rounds.
    blur_steps: vec4<f32>,
    rounds: f32,
    on: f32,
}

@group(0) @binding(0) var frame: texture_2d<f32>;
@group(0) @binding(1) var clamped: sampler;
@group(0) @binding(2) var<uniform> fx: Visor;
@group(0) @binding(3) var mask_tex: texture_2d<f32>;
@group(0) @binding(4) var scratches_tex: texture_2d<f32>;
@group(0) @binding(5) var blur_mask_tex: texture_2d<f32>;
@group(0) @binding(6) var distort_tex: texture_2d<f32>;
@group(0) @binding(7) var blurred: texture_2d<f32>;
@group(0) @binding(8) var repeated: sampler;

struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    // Which round of the blur a draw is (its instance).
    @location(1) @interpolate(flat) round: u32,
}

// The whole target, as Bevy's full-screen triangle, with the draw's round.
@vertex
fn vertex(@builtin(vertex_index) index: u32, @builtin(instance_index) round: u32) -> Out {
    var out: Out;
    let uv = vec2<f32>(f32(index >> 1u), f32(index & 1u)) * 2.0;
    out.position = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    out.uv = uv;
    out.round = round;
    return out;
}

// The game's place of a pixel (from the lower left).
fn unity(uv: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(uv.x, 1.0 - uv.y);
}

fn frame_at(s: vec2<f32>) -> vec4<f32> {
    return textureSampleLevel(frame, clamped, vec2<f32>(s.x, 1.0 - s.y), 0.0);
}

// The mask's place (every vertex program): the screen turned by the
// rotation about its middle, zoomed by `1 - |hit zoom| - c`, swung, and
// slid down from the top by the intensity.
fn mask_space(s: vec2<f32>, c: f32) -> vec2<f32> {
    let d = s - 0.5;
    let turned = vec2<f32>(cos(fx.rotation) * d.x + sin(fx.rotation) * d.y, -sin(fx.rotation) * d.x + cos(fx.rotation) * d.y);
    let z = 0.5 + (1.0 - abs(fx.hit_zoom) - c) * turned + fx.swing;
    return vec2<f32>(z.x, z.y + fx.intensity - 1.0);
}

// Whether a place is behind the shield, and its lower edge's fade (where
// the mask's y is under 0.1: `y / _EdgeFade`, at most 1; with no fade, as
// the shield is all the way down, 1).
fn coverage(m: vec2<f32>) -> f32 {
    if m.y <= 0.0 {
        return 0.0;
    }
    if m.y < 0.1 && fx.edge_fade > 0.0 {
        return min(m.y / fx.edge_fade, 1.0);
    }
    return 1.0;
}

// The result's colour with the frame's own alpha: the game's passes blend
// and mask the alpha with the colour, which nothing shows (the screenshot
// would show it as see-through).
fn keep_alpha(c: vec4<f32>, out: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(out.rgb, c.a);
}

// Pass 0: the scratches over the frame, their colour taken by the light, at
// their own alpha by the scratches' intensity; a flash takes them away.
@fragment
fn scratches(in: Out) -> @location(0) vec4<f32> {
    let s = unity(in.uv);
    let c = textureSampleLevel(frame, clamped, in.uv, 0.0);
    let m = mask_space(s, 0.1);
    let k = textureSampleLevel(scratches_tex, repeated, m, 0.0) * fx.textured;
    let laid = mix(c, vec4<f32>(k.rgb * fx.light, k.a), (1.0 - fx.blindness) * k.a * fx.scratches_intensity);
    return keep_alpha(c, c + coverage(m) * (laid - c));
}

// Pass 2: the frame moved by the glass's distortion, nothing at the middle
// and most at the corners (twice the squared distance from the middle).
@fragment
fn distort(in: Out) -> @location(0) vec4<f32> {
    let s = unity(in.uv);
    let c = textureSampleLevel(frame, clamped, in.uv, 0.0);
    let m = mask_space(s, 0.1);
    let unzoomed = mask_space(s, 0.0);
    let d = textureSampleLevel(distort_tex, clamped, unzoomed, 0.0).rg * fx.textured;
    let off = s - 0.5;
    let moved = frame_at(clamp(s + d * fx.distort_intensity * 2.0 * dot(off, off), vec2<f32>(0.0), vec2<f32>(1.0)));
    return keep_alpha(c, c + coverage(m) * (moved - c));
}

// The fast blur's first pass: four taps of the whole frame into a half of
// its size, a texel off and half a texel off (the game's, turned for
// Bevy's y down).
@fragment
fn blur_down(in: Out) -> @location(0) vec4<f32> {
    let t = 1.0 / vec2<f32>(textureDimensions(frame));
    let a = textureSampleLevel(frame, clamped, in.uv + vec2<f32>(1.0, -1.0) * t, 0.0);
    let b = textureSampleLevel(frame, clamped, in.uv + vec2<f32>(-0.5, 0.5) * t, 0.0);
    let c = textureSampleLevel(frame, clamped, in.uv + vec2<f32>(0.5, 0.5) * t, 0.0);
    let d = textureSampleLevel(frame, clamped, in.uv + vec2<f32>(-0.5, -0.5) * t, 0.0);
    return (a + b + c + d) * 0.25;
}

const WEIGHTS = array<f32, 7>(0.0205, 0.0855, 0.232, 0.324, 0.232, 0.0855, 0.0205);

// Seven taps along a way, the round's step apart; the alpha the middle's.
fn seven(uv: vec2<f32>, step: vec2<f32>) -> vec4<f32> {
    var sum = vec3<f32>(0.0);
    var middle = 0.0;
    for (var k = 0; k < 7; k++) {
        let tap = textureSampleLevel(frame, clamped, uv + f32(k - 3) * step, 0.0);
        sum += tap.rgb * WEIGHTS[k];
        if k == 3 {
            middle = tap.a;
        }
    }
    return vec4<f32>(sum, middle);
}

// The fast blur's second and third passes: up and down, then across, by
// the round's `_Parameter.x` in half-size texels.
@fragment
fn blur_vertical(in: Out) -> @location(0) vec4<f32> {
    let t = 1.0 / vec2<f32>(textureDimensions(frame));
    return seven(in.uv, vec2<f32>(0.0, fx.blur_steps[min(in.round, 3u)] * t.y));
}

@fragment
fn blur_across(in: Out) -> @location(0) vec4<f32> {
    let t = 1.0 / vec2<f32>(textureDimensions(frame));
    return seven(in.uv, vec2<f32>(fx.blur_steps[min(in.round, 3u)] * t.x, 0.0));
}

// Pass 3: the blur where the blur mask says, and the frame's mask (taken at
// the mask's place turned top to bottom, which its repeating wrap makes of
// a place below nothing).
@fragment
fn framed(in: Out) -> @location(0) vec4<f32> {
    let s = unity(in.uv);
    let c = textureSampleLevel(frame, clamped, in.uv, 0.0);
    let m = mask_space(s, 0.1);
    let b = textureSampleLevel(blur_mask_tex, repeated, m, 0.0).a * fx.textured;
    let soft = textureSampleLevel(blurred, clamped, in.uv, 0.0);
    let held = mix(1.0, textureSampleLevel(mask_tex, repeated, vec2<f32>(m.x, -m.y), 0.0).a, fx.framed);
    let x = mix(c, soft, b) * held;
    return keep_alpha(c, c + coverage(m) * (x - c));
}
