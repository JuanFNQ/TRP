// The world blurred behind the inventory (`screen_blur.rs`): a copy by one
// linear sample, and the mix of the sharp picture with the blurred square.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct Blur {
    value: f32,
}

@group(0) @binding(0) var picture: texture_2d<f32>;
@group(0) @binding(1) var linear_sampler: sampler;
@group(0) @binding(2) var blurred: texture_2d<f32>;
@group(0) @binding(3) var<uniform> blur: Blur;

@fragment
fn copy(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    return textureSample(picture, linear_sampler, in.uv);
}

@fragment
fn mix_blurred(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let sharp = textureSample(picture, linear_sampler, in.uv);
    let soft = textureSample(blurred, linear_sampler, in.uv);
    return blur.value * (soft - sharp) + sharp;
}
