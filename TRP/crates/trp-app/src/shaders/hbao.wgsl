// The game's ambient occlusion, its `HBAO` (`hbao_view.rs`;
// docs/spec/ambient-occlusion.md): the occlusion, the blur across, the blur
// down and the composite, each drawn over the whole frame. Places on the
// screen are the game's: `u` from the left, `v` from the bottom; the view's
// x to the right, y down, z forward.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct Hbao {
    // The camera's view from the world (the normals turned by it).
    view_from_world: mat4x4<f32>,
    // The camera's view from its clip (each depth turned back by it).
    view_from_clip: mat4x4<f32>,
    // The frame's width and height, and their inverses.
    size: vec4<f32>,
    // The blur's first texture's width and height, and their inverses.
    across_size: vec4<f32>,
    // `_UVToView`.
    uv_to_view: vec4<f32>,
    // `_Radius`, `_MaxRadiusPixels`, `_NegInvRadius2`, `_AngleBias`.
    reach: vec4<f32>,
    // `_AOmultiplier`, `_MaxDistance`, `_DistanceFalloff`, the far plane.
    distance: vec4<f32>,
    // Directions, steps, 1 / (steps + 1), 1 / (directions × steps).
    march: vec4<f32>,
    // The blur's taps on either side, their fall-off, the sharpness.
    blur: vec4<f32>,
    // The intensity, then the base colour.
    composite: vec4<f32>,
    // Each direction's turn from the noise's: cosine, sine.
    turns: array<vec4<f32>, 8>,
}

@group(0) @binding(0) var<uniform> hbao: Hbao;

// --- The occlusion ---------------------------------------------------------

#ifdef OCCLUSION
@group(1) @binding(0) var scene_depth: texture_2d<f32>;
#ifdef MULTISAMPLED
@group(1) @binding(1) var normals: texture_multisampled_2d<f32>;
#else
@group(1) @binding(1) var normals: texture_2d<f32>;
#endif
@group(1) @binding(2) var noise: texture_2d<f32>;

// The distance along the view at a texel of the frame (`top` counted from
// the top), an edge's for one off the frame, as the game's clamped read; the
// cleared depth is the far plane's, as the game's depth there is.
fn view_depth(across: i32, top: i32) -> f32 {
    let size = vec2<i32>(hbao.size.xy);
    let at = clamp(vec2<i32>(across, top), vec2<i32>(0), size - vec2<i32>(1));
    let d = textureLoad(scene_depth, at, 0).r;
    if d <= 0.0 {
        return hbao.distance.w;
    }
    let ndc = vec2<f32>((f32(at.x) + 0.5) * hbao.size.z * 2.0 - 1.0, 1.0 - (f32(at.y) + 0.5) * hbao.size.w * 2.0);
    let v = hbao.view_from_clip * vec4<f32>(ndc, d, 1.0);
    return min(-v.z / v.w, hbao.distance.w);
}

fn view_place(uv: vec2<f32>, z: f32) -> vec3<f32> {
    return vec3<f32>((uv * hbao.uv_to_view.xy + hbao.uv_to_view.zw) * z, z);
}

@fragment
fn occlusion(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let px = vec2<i32>(in.position.xy);
    let z = view_depth(px.x, px.y);
    // Beyond the maximum distance the game's pass keeps nothing: the texture
    // stays as cleared, white.
    if !(hbao.distance.y - z >= 0.0) {
        return vec4<f32>(1.0);
    }
    // The game's place on the screen: `v` from the bottom.
    let uv = vec2<f32>(in.position.x * hbao.size.z, 1.0 - in.position.y * hbao.size.w);
    let p = view_place(uv, z);
    // The first sample's, as the depth's copy keeps (`scene_depth.wgsl`).
    let encoded = textureLoad(normals, px, 0).xyz;
    let world_normal = encoded * 2.0 - 1.0;
    let n = (hbao.view_from_world * vec4<f32>(world_normal, 0.0)).xyz * vec3<f32>(1.0, -1.0, -1.0);

    let reach = min(hbao.reach.x / z, hbao.reach.y);
    let step = reach * hbao.march.z;
    // The noise by whole texels, its rows from the bottom.
    let bottom = i32(hbao.size.y) - 1 - px.y;
    let r = textureLoad(noise, vec2<i32>(px.x & 3, bottom & 3), 0).xyz;
    let first = r.z * step + 1.0;

    let directions = i32(hbao.march.x);
    let steps = i32(hbao.march.y);
    var sum = 0.0;
    for (var d = 0; d < directions; d++) {
        let t = hbao.turns[d].xy;
        let dir = vec2<f32>(t.x * r.x - t.y * r.y, t.x * r.y + t.y * r.x);
        var ray = first;
        for (var s = 0; s < steps; s++) {
            // Whole pixels off, rounded to the nearest (a half to the even).
            let off = round(dir * ray);
            let zs = view_depth(px.x + i32(off.x), px.y - i32(off.y));
            let v = view_place(uv + off * hbao.size.zw, zs) - p;
            let vv = dot(v, v);
            // A sample on the pixel itself adds nothing (the game's sum of
            // nought times the infinite is kept as nought).
            if vv > 0.0 {
                sum += saturate(dot(n, v) * inverseSqrt(vv) - hbao.reach.w) * saturate(vv * hbao.reach.z + 1.0);
            }
            ray += step;
        }
    }
    var ao = saturate(1.0 - sum * hbao.distance.x * hbao.march.w);
    let start = hbao.distance.y - hbao.distance.z;
    let fade = saturate((z - start) / (hbao.distance.y - start));
    ao = fade * (1.0 - ao) + ao;

    // The depth as a share of the far plane in two bytes.
    let share = saturate(z * (1.0 / hbao.distance.w));
    let fine = fract(share * 255.0);
    return vec4<f32>(fract(share) - fine * (1.0 / 255.0), fine, 1.0, ao);
}
#endif

// --- The blur ---------------------------------------------------------------

#ifdef BLUR
@group(1) @binding(0) var blurred: texture_2d<f32>;
@group(1) @binding(1) var blurred_sampler: sampler;

fn stored_depth(c: vec4<f32>) -> f32 {
    return dot(c.xy, vec2<f32>(1.0, 1.0 / 255.0));
}

// A weighted mean along `texel`, the middle weighing 1, the taps by their
// place and their depth's difference from the middle's.
fn blur(uv: vec2<f32>, texel: vec2<f32>) -> vec4<f32> {
    let middle = textureSampleLevel(blurred, blurred_sampler, uv, 0.0);
    let depth = stored_depth(middle);
    var sum = middle.w;
    var total = 1.0;
    let reach = i32(hbao.blur.x);
    for (var side = -1; side <= 1; side += 2) {
        for (var k = 1; k <= reach; k++) {
            let tap = textureSampleLevel(blurred, blurred_sampler, uv + f32(side * k) * texel, 0.0);
            let difference = (stored_depth(tap) - depth) * hbao.distance.w * hbao.blur.z;
            let w = exp2(-f32(k * k) * hbao.blur.y - difference * difference);
            sum += w * tap.w;
            total += w;
        }
    }
    return vec4<f32>(middle.xy, 1.0, sum / total);
}

// Across, in the first texture's texels.
@fragment
fn blur_across(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    return blur(in.uv, vec2<f32>(hbao.across_size.z, 0.0));
}

// Down, in the frame's.
@fragment
fn blur_down(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    return blur(in.uv, vec2<f32>(0.0, hbao.size.w));
}
#endif

// --- The composite ----------------------------------------------------------

#ifdef COMPOSITE
@group(1) @binding(0) var frame: texture_2d<f32>;
@group(1) @binding(1) var occluded: texture_2d<f32>;
@group(1) @binding(2) var occluded_sampler: sampler;

@fragment
fn composite(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let ao = textureSampleLevel(occluded, occluded_sampler, in.uv, 0.0).w;
    let f = min(exp2(log2(ao) * hbao.composite.x), 1.0);
    let base = hbao.composite.yzw;
    let tint = base + f * (vec3<f32>(1.0) - base);
#ifdef AO_ONLY
    // The game's display of the occlusion alone (`DisplayMode.AOOnly`).
    return vec4<f32>(tint, 1.0);
#else
    let c = textureLoad(frame, vec2<i32>(in.position.xy), 0);
    return vec4<f32>(c.rgb * tint, c.a);
#endif
}
#endif
