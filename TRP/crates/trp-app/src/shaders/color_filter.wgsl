// The camera's colour filter (`color_filter_view.rs`; lighting spec, "The
// camera's colour filter"): `Hidden/CC_LookupFilter`'s gamma pass, on the
// frame in the game's numbers after PRISM. The colour held between nought
// and one; its blue picks two of the table's 64 tiles (8 by 8; the higher
// and the lower level), its red and green a place within each, the two
// blended by the blue's fraction; the result taken toward by the amount.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct ColorFilter {
    // The amount (x).
    figures: vec4<f32>,
}

@group(0) @binding(0) var picture: texture_2d<f32>;
@group(0) @binding(1) var picture_sampler: sampler;
@group(0) @binding(2) var table: texture_2d<f32>;
@group(0) @binding(3) var table_sampler: sampler;
@group(0) @binding(4) var<uniform> figures: ColorFilter;

// A tile's place in the table, and the red's and green's within it: the
// game's sum, its second coordinate turned round (the table read with its
// rows repeated, as its texture is).
fn tile(level: f32, rg: vec2<f32>) -> vec2<f32> {
    let row = floor(level * 0.125);
    let column = level - row * 8.0;
    return vec2<f32>(column * 0.125 + rg.x, -(row * 0.125 + rg.y));
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let c = clamp(textureSample(picture, picture_sampler, in.uv), vec4<f32>(0.0), vec4<f32>(1.0));
    let blue = c.b * 63.0;
    // 63/512 and half a texel of 512.
    let rg = c.rg * 0.123046875 + vec2<f32>(0.0009765625);
    let upper = textureSample(table, table_sampler, tile(ceil(blue), rg)).rgb;
    let lower = textureSample(table, table_sampler, tile(floor(blue), rg)).rgb;
    let graded = mix(lower, upper, fract(blue));
    return vec4<f32>(c.rgb + figures.figures.x * (graded - c.rgb), c.a);
}
