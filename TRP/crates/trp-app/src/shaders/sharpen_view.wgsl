// The camera's sharpening and desaturation (`sharpen_view.rs`; health spec,
// "Desaturation"), as the game's compiled `Hidden/CC_Sharpen` does them with
// `CC_SHARPEN_DESATURATE` on: the picture sharpened by its brightness less a
// quarter of four taps about it (held to the clamp), then turned toward the
// ramp's colour of its brightness by the weather's and the health's share
// (held to 0 to 1 together), and outside the painkiller's ring by three
// quarters of the way, times the mask; night vision's green tint on the
// ring's colour (none here). The game's texture coordinates run from the
// lower left: a place is taken so and turned to Bevy's to sample.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct Sharpen {
    // Its strength, clamp, the ring's falloff and night vision's ratio
    // (`_Strength`, `_Clamp`, `_RadiusFalloff`, `_NightVisionRatio`).
    figures: vec4<f32>,
    // The weather's, the health's and the mask's desaturation, the ring's
    // radius (`_DesatWeather`, `_DesatHealth`, `_DesatMask`, `_Radius`).
    desaturate: vec4<f32>,
    // `_RampOffset`.
    ramp_offset: vec4<f32>,
}

@group(0) @binding(0) var picture: texture_2d<f32>;
@group(0) @binding(1) var linear_sampler: sampler;
@group(0) @binding(2) var<uniform> fx: Sharpen;
@group(0) @binding(3) var ramp: texture_2d<f32>;

// The picture at a place given from its lower left.
fn at(p: vec2<f32>) -> vec4<f32> {
    return textureSample(picture, linear_sampler, vec2<f32>(p.x, 1.0 - p.y));
}

@fragment
fn sharpen(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let uv = vec2<f32>(in.uv.x, 1.0 - in.uv.y);
    let size = vec2<f32>(textureDimensions(picture));
    // `_PX`, `_PY`: a pixel of the screen.
    let px = 1.0 / size;
    // The ring: how far out from the middle, across by the screen's
    // aspect, past the radius over the falloff, smoothed.
    let d = length(vec2<f32>((uv.x - 0.5) * size.x / size.y, uv.y - 0.5));
    let t = clamp((d - fx.desaturate.w) / fx.figures.z, 0.0, 1.0);
    let ring = t * t * (3.0 - 2.0 * t);
    let around = at(uv + px * vec2<f32>(0.5, -1.0)).rgb + at(uv + px * vec2<f32>(-1.0, -0.5)).rgb + at(uv + px * vec2<f32>(1.0, 0.5)).rgb + at(uv + px * vec2<f32>(-0.5, 1.0)).rgb;
    let here = at(uv);
    let edge = dot(here.rgb - around * 0.25, fx.figures.x * vec3<f32>(0.141592, 0.476323, 0.048085));
    let c = here + vec4<f32>(clamp(edge, -fx.figures.y, fx.figures.y));
    let grey = textureSample(ramp, linear_sampler, vec2<f32>(dot(c.rgb, vec3<f32>(0.22, 0.707, 0.071)), 0.5)).rgb;
    let mask = fx.desaturate.z;
    let ringed = vec4<f32>(c.rgb * (1.0 - mask) + (c.rgb + (grey - c.rgb) * 0.75) * mask + fx.ramp_offset.rgb, c.a);
    let whole = clamp(fx.desaturate.y + fx.desaturate.x, 0.0, 1.0);
    let base = vec4<f32>(c.rgb * (1.0 - whole) + grey * whole + fx.ramp_offset.rgb, c.a);
    let tint = fx.figures.w * vec4<f32>(1.0, 0.5, 1.0, 0.0) + vec4<f32>(0.0, 0.5, 0.0, 1.0);
    return base + ring * mask * (ringed * tint - base);
}
