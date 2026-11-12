// The dust drawn off the frame (`offscreen_dust.rs`, the options' DUST
// RESOLUTION; particles spec, "Drawn off the frame"): the scene's depth made
// smaller for the dust to be tested against, and the dust laid back over the
// frame. The off-frame picture holds, for each of its pixels, the colour the
// dust adds and how much of the frame shows through it (its alpha), so that
// laying it over is the same sum as the dust's draws blended one by one.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct Dust {
    // The camera's near plane, the reduction (full pixels across a small
    // one), the depth's share past which the nearest sample is taken.
    figures: vec4<f32>,
}

@group(0) @binding(0) var<uniform> dust: Dust;
@group(0) @binding(1) var full_depth: texture_2d<f32>;
@group(0) @binding(2) var small_depth: texture_2d<f32>;
@group(0) @binding(3) var small_colour: texture_2d<f32>;
@group(0) @binding(4) var bilinear: sampler;

// The depth from the eye of a reversed depth (the app's, infinitely far at
// nought), held short of infinity for the comparisons.
fn eye(d: f32) -> f32 {
    return min(dust.figures.x / max(d, 1e-12), 1e6);
}

// Each small pixel's depth: the nearest of the full pixels under it, so that
// dust behind anything in the block is not drawn there.
@fragment
fn downsample(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let n = i32(dust.figures.y);
    let base = vec2<i32>(in.position.xy) * n;
    let most = vec2<i32>(textureDimensions(full_depth)) - vec2<i32>(1);
    var nearest = 0.0;
    for (var y = 0; y < n; y++) {
        for (var x = 0; x < n; x++) {
            nearest = max(nearest, textureLoad(full_depth, min(base + vec2<i32>(x, y), most), 0).r);
        }
    }
    return vec4<f32>(nearest, 0.0, 0.0, 1.0);
}

// Laid over as NVIDIA's nearest-depth upsampling does: where the four small
// pixels around this one lie at about this pixel's depth, read between them;
// where any lies farther off (an edge of the scene), the one nearest this
// pixel's depth alone.
@fragment
fn upsample(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let small = vec2<f32>(textureDimensions(small_colour));
    let z = eye(textureLoad(full_depth, vec2<i32>(in.position.xy), 0).r);
    let at = in.uv * small - vec2<f32>(0.5);
    let base = vec2<i32>(floor(at));
    let most = vec2<i32>(small) - vec2<i32>(1);
    var best = clamp(base, vec2<i32>(0), most);
    var best_gap = 1e30;
    var widest = 0.0;
    for (var k = 0; k < 4; k++) {
        let q = clamp(base + vec2<i32>(k & 1, k >> 1), vec2<i32>(0), most);
        let gap = abs(eye(textureLoad(small_depth, q, 0).r) - z);
        widest = max(widest, gap);
        if gap < best_gap {
            best_gap = gap;
            best = q;
        }
    }
    if widest <= dust.figures.z * z {
        return textureSampleLevel(small_colour, bilinear, in.uv, 0.0);
    }
    return textureLoad(small_colour, best, 0);
}
