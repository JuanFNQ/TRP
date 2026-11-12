// The camera's distortion (`distort_view`; firing spec, "The muzzle's heat
// haze"), as the game's `Hidden/DistortCameraFX` draws it: the frame copied
// at half size and blurred across and down (seven taps, its pass 0), and the
// frame moved by the distortion values' red and green (`_Distortion` pixels
// at full) toward the blurred copy by their blue (its pass 1).

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct Distort {
    // `_Distortion` (pixels over the frame's size), and the blur's step
    // across and down (one half-size pixel times two to the iteration).
    distortion: vec4<f32>,
    step: vec4<f32>,
}

@group(0) @binding(0) var<uniform> distort: Distort;
@group(0) @binding(1) var source: texture_2d<f32>;
@group(0) @binding(2) var linear_sampler: sampler;
@group(0) @binding(3) var values: texture_2d<f32>;
@group(0) @binding(4) var blurred: texture_2d<f32>;

// `Graphics.Blit` to the half-size copy: the frame read between its
// pixels.
@fragment
fn half(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    return textureSample(source, linear_sampler, in.uv);
}

fn blur(uv: vec2<f32>, step: vec2<f32>) -> vec4<f32> {
    return textureSample(source, linear_sampler, uv) * 0.28
        + (textureSample(source, linear_sampler, uv + step) + textureSample(source, linear_sampler, uv - step)) * 0.22
        + (textureSample(source, linear_sampler, uv + 2.0 * step) + textureSample(source, linear_sampler, uv - 2.0 * step)) * 0.10
        + (textureSample(source, linear_sampler, uv + 3.0 * step) + textureSample(source, linear_sampler, uv - 3.0 * step)) * 0.04;
}

@fragment
fn blur_across(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    return blur(in.uv, vec2<f32>(distort.step.x, 0.0));
}

@fragment
fn blur_down(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    return blur(in.uv, vec2<f32>(0.0, distort.step.y));
}

// The frame moved by the values toward the blurred copy (`DistortCameraFX`
// pass 1).
@fragment
fn composite(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let d = textureSample(values, linear_sampler, in.uv);
    let at = (d.xy * 2.0 - vec2<f32>(1.0)) * distort.distortion.xy + in.uv;
    let frame = textureSample(source, linear_sampler, in.uv);
    return mix(frame, textureSample(blurred, linear_sampler, at), d.z);
}
