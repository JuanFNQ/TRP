// The tactical lights' lens flares (`flare_view`; tactical-devices spec, "The
// light shaft and the flare"), as the game's compiled `Custom/multiFlare`
// (the normal and flashlight flares) and `Custom/ShitOnScreen` (the noise
// flares) draw them: each flare a quad facing the screen at its light's place
// moved toward the camera, its corners worked out per flare as the game's
// geometry shader does (here in the vertex stage, six corners a flare), added
// to the frame (`Blend One One`), no depth test, both faces.

struct Flares {
    clip_from_world: mat4x4<f32>,
    view_from_world: mat4x4<f32>,
    clip_from_view: mat4x4<f32>,
    // The camera's place, and `_Time.x` (seconds over 20).
    camera: vec4<f32>,
    // `Custom/multiFlare`: `_DepthOffset`, unused, `_FallofShift`,
    // `_FallofStrength`; then `_VisibilityCheckerSize`, `_Intensity`.
    multi: vec4<f32>,
    multi_b: vec4<f32>,
    // `Custom/ShitOnScreen`: `_DepthOffset`, `_FadeDepthOffset`,
    // `_FallofShift`, `_FallofStrength`; then `_VisibilityCheckerSize`,
    // `_Intensity`, `_NoiseIntensity`; then `_NoiseScale`.
    noise: vec4<f32>,
    noise_b: vec4<f32>,
    noise_scale: vec4<f32>,
}

@group(0) @binding(0) var<uniform> flares: Flares;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;
@group(0) @binding(3) var grain: texture_2d<f32>;
@group(0) @binding(4) var grain_sampler: sampler;
@group(0) @binding(5) var scene_depth: texture_2d<f32>;

struct Corner {
    // Its light's place (Bevy's axes).
    @location(0) position: vec3<f32>,
    // Which corner, each of −1 or 1.
    @location(1) corner: vec2<f32>,
    // Its picture's corner and size in the atlas (v up).
    @location(2) uv: vec4<f32>,
    @location(3) scale: vec2<f32>,
    // Its colour, its fourth the light's alpha.
    @location(4) color: vec4<f32>,
    // The least distance, one over the range, the size's and alpha's ranges.
    @location(5) dist: vec4<f32>,
    // `FlareType`: 2 the noise flare, else the normal and flashlight.
    @location(6) kind: f32,
}

struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) noise_uv: vec2<f32>,
    @location(2) silhouette: f32,
    @location(3) color: vec4<f32>,
    @location(4) @interpolate(flat) noise: u32,
}

// The 23 taps around the point that count how much of it is clear.
const TAPS = array<vec2<f32>, 23>(
    vec2<f32>(-0.668, -0.632), vec2<f32>(-0.359, -0.839), vec2<f32>(-0.175, -0.498), vec2<f32>(0.612, -0.636),
    vec2<f32>(-0.444, -0.297), vec2<f32>(0.151, -0.402), vec2<f32>(0.205, -0.849), vec2<f32>(0.847, -0.317),
    vec2<f32>(-0.858, -0.263), vec2<f32>(-0.098, -0.041), vec2<f32>(0.297, 0.093), vec2<f32>(0.517, -0.220),
    vec2<f32>(-0.839, 0.031), vec2<f32>(-0.824, 0.361), vec2<f32>(-0.507, 0.076), vec2<f32>(0.002, 0.423),
    vec2<f32>(0.351, 0.497), vec2<f32>(0.756, 0.108), vec2<f32>(0.766, 0.524), vec2<f32>(-0.124, 0.878),
    vec2<f32>(-0.536, 0.724), vec2<f32>(0.288, 0.895), vec2<f32>(-0.373, 0.397),
);

// The scene's depth (Bevy's: reversed, 1 at the near plane) at a place on
// the screen, held to its edges (`PROVISIONAL`: the engine's sampler).
fn depth_at(uv: vec2<f32>) -> f32 {
    let size = vec2<i32>(textureDimensions(scene_depth));
    let pixel = clamp(vec2<i32>(floor(uv * vec2<f32>(size))), vec2<i32>(0), size - vec2<i32>(1));
    return textureLoad(scene_depth, pixel, 0).r;
}

// A place on the screen as the game's shaders take it from the clip's
// numbers: x right, y down.
fn screen_of(ndc: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(ndc.x * 0.5 + 0.5, 1.0 - (ndc.y * 0.5 + 0.5));
}

// A flare not drawn: its corners outside the clip volume.
fn cut() -> Out {
    var out: Out;
    out.clip = vec4<f32>(0.0, 0.0, -1.0, 1.0);
    return out;
}

fn projection() -> vec2<f32> {
    return vec2<f32>(flares.clip_from_view[0][0], flares.clip_from_view[1][1]);
}

// The screen edge's fade: each way, held within 0 and 1.
fn edge(ndc: vec2<f32>, shift: f32, strength: f32) -> f32 {
    let e = clamp((vec2<f32>(shift) - abs(ndc)) * strength, vec2<f32>(0.0), vec2<f32>(1.0));
    return e.x * e.y;
}

// `Custom/multiFlare`'s geometry stage.
fn multi(c: Corner) -> Out {
    let to_camera = normalize(flares.camera.xyz - c.position);
    let p = c.position + to_camera * flares.multi.x;
    let clip = flares.clip_from_world * vec4<f32>(p, 1.0);
    // Its distance (`w − z`: the view depth less the near plane).
    let dist = clip.w - clip.z;
    if (c.color.w < 0.01 || clip.x < -clip.w || clip.y < -clip.w || clip.w < clip.x || clip.w < clip.y || dist < -clip.w || clip.w < dist) {
        return cut();
    }
    // Its depth as the game's shader takes it: 0 at the near plane.
    let ndc = vec3<f32>(clip.x, clip.y, dist) / clip.w;
    let at = screen_of(ndc.xy);
    let r = projection() * flares.multi_b.x / ndc.z;
    var count = 0.0;
    for (var i = 0u; i < 23u; i++) {
        if (ndc.z < 1.0 - depth_at(at + r * TAPS[i])) {
            count += 1.0;
        }
    }
    let e = edge(ndc.xy, flares.multi.z, flares.multi.w);
    if (count < 0.23 || e < 0.01) {
        return cut();
    }
    let visible = count * 0.043478;
    let t = clamp((dist - c.dist.x) * c.dist.y, 0.0, 1.0);
    let alpha_factor = 1.0 + c.dist.w * t;
    let size_factor = 1.0 + c.dist.z * t;
    let half = projection() * c.scale * dist * visible * size_factor;
    var out: Out;
    out.clip = vec4<f32>(clip.xy + c.corner * half, clip.z, clip.w);
    out.uv = c.uv.xy + (c.corner * 0.5 + 0.5) * c.uv.zw;
    out.color = alpha_factor * c.color * e * c.color.w * visible;
    out.silhouette = 1.0;
    out.noise = 0u;
    return out;
}

// Whether a probe of the noise flare's silhouette is clear: the scene not
// nearer there than it.
fn clear(view: vec4<f32>, offset: vec2<f32>) -> bool {
    let clip = flares.clip_from_view * (view + vec4<f32>(offset, 0.0, 0.0));
    let ndc = clip.xyz / clip.w;
    return ceil(clamp(depth_at(screen_of(ndc.xy)) - ndc.z, 0.0, 1.0)) == 0.0;
}

// `Custom/ShitOnScreen`'s geometry stage (its depth taken as the game's
// compiled shader takes it, two of its tests not turned for the reversed
// depth: its taps nearly all count, and its distance is the clip's z, the
// near plane's).
fn noise(c: Corner) -> Out {
    let to_camera = normalize(flares.camera.xyz - c.position);
    let p = c.position + to_camera * flares.noise.x;
    let clip = flares.clip_from_world * vec4<f32>(p, 1.0);
    if (clip.x < -clip.w || clip.y < -clip.w || clip.z < -clip.w || clip.w < clip.x || clip.w < clip.y || clip.w < clip.z) {
        return cut();
    }
    let ndc = clip.xyz / clip.w;
    let at = screen_of(ndc.xy);
    let r = projection() * flares.noise_b.x / ndc.z;
    var count = 0.0;
    for (var i = 0u; i < 23u; i++) {
        if (ndc.z < 1.0 - depth_at(at + r * TAPS[i])) {
            count += 1.0;
        }
    }
    let e = edge(ndc.xy, flares.noise.z, flares.noise.w);
    if (count < 0.23 || e < 0.01) {
        return cut();
    }
    // The silhouette: probes 1.25 cm apart, seven each way along the view's
    // x and y, from a point 0.4 m toward the camera; each way the nearest
    // clear one's eighth (1 where none is), the two ways multiplied, where
    // the scene hides the point itself.
    let view = flares.view_from_world * vec4<f32>(c.position + to_camera * flares.noise.y, 1.0);
    var ways = vec2<f32>(1.0);
    for (var axis = 0u; axis < 2u; axis++) {
        let along = select(vec2<f32>(0.0, 0.0125), vec2<f32>(0.0125, 0.0), axis == 0u);
        var one = 1.0;
        for (var k = 7; k >= 1; k--) {
            if (clear(view, along * f32(k))) {
                one = 0.125 * f32(k);
            }
        }
        var both = one;
        for (var k = 7; k >= 1; k--) {
            if (clear(view, -along * f32(k))) {
                both = 0.125 * f32(k);
            }
        }
        ways[axis] = min(both, one);
    }
    let hidden = ceil(clamp(depth_at(at) - ndc.z, 0.0, 1.0));
    let silhouette = 1.0 - ways.x * ways.y * hidden;
    if (silhouette < 0.01) {
        return cut();
    }
    let visible = count * 0.043478;
    let dist = clip.z;
    let t = clamp((dist - c.dist.x) * c.dist.y, 0.0, 1.0);
    let alpha_factor = 1.0 + c.dist.w * t;
    let size_factor = 1.0 + c.dist.z * t;
    let half = projection() * c.scale * dist * silhouette * size_factor;
    var out: Out;
    let placed = clip.xy + c.corner * half;
    out.clip = vec4<f32>(placed, clip.z, clip.w);
    out.uv = c.uv.xy + (c.corner * 0.5 + 0.5) * c.uv.zw;
    let t_x = flares.camera.w;
    out.noise_uv = screen_of(placed / clip.w) * flares.noise_scale.xy + vec2<f32>(fract(t_x * 14345.678), fract(-t_x * 12345.678));
    out.color = alpha_factor * c.color * e * c.color.w * visible * silhouette;
    out.silhouette = silhouette;
    out.noise = 1u;
    return out;
}

@vertex
fn vertex(c: Corner) -> Out {
    if (c.kind == 2.0) {
        return noise(c);
    }
    return multi(c);
}

@fragment
fn fragment(in: Out) -> @location(0) vec4<f32> {
    // Both read first: a sample may not wait on a branch by the pixel.
    let picture = textureSample(atlas, atlas_sampler, in.uv);
    let speck = textureSample(grain, grain_sampler, in.noise_uv).r;
    if (in.noise == 1u) {
        return speck * flares.noise_b.z * picture * flares.noise_b.y * in.color * in.silhouette;
    }
    return picture * in.color * flares.multi_b.y;
}
