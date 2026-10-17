// The frame turned into its gamma-encoded numbers before the screens are
// drawn over it, and back after them (`gamma_ui.rs`): the sRGB curve.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var picture: texture_2d<f32>;
@group(0) @binding(1) var nearest: sampler;

@fragment
fn encode(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let c = textureSample(picture, nearest, in.uv);
    let v = max(c.rgb, vec3(0.0));
    return vec4(select(1.055 * pow(v, vec3(1.0 / 2.4)) - 0.055, v * 12.92, v <= vec3(0.0031308)), c.a);
}

@fragment
fn decode(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let c = textureSample(picture, nearest, in.uv);
    let v = max(c.rgb, vec3(0.0));
    return vec4(select(pow((v + 0.055) / 1.055, vec3(2.4)), v / 12.92, v <= vec3(0.04045)), c.a);
}
