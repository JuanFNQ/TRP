// The inspect window's preview's photographic tonemap (`preview_tonemap.rs`):
// Standard Assets' `Tonemapping` of the type 3, `1 - 2^(-exposure c)`, on
// the frame's figures as the game's numbers (the 2.2 power) and back; the
// alpha, the picture's cover, kept.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct Tonemap {
    // The exposure (x).
    figures: vec4<f32>,
}

@group(0) @binding(0) var picture: texture_2d<f32>;
@group(0) @binding(1) var picture_sampler: sampler;
@group(0) @binding(2) var<uniform> tonemap: Tonemap;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let c = textureSample(picture, picture_sampler, in.uv);
    let numbers = pow(max(c.rgb, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.2));
    let mapped = vec3<f32>(1.0) - exp2(-tonemap.figures.x * numbers);
    return vec4<f32>(pow(mapped, vec3<f32>(2.2)), c.a);
}
