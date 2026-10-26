// A contusion's screen effects (`contusion_view.rs`; health spec, "A
// contusion's screen effects"), as the game's compiled shaders do them:
// `Hidden/CC_DoubleVision`, `Hidden/CC_Wiggle`, and `Hidden/MotionBlur`'s
// colour pass; and health's others (health spec, "A hit and health's other
// screen effects"): `Hidden/CC_RadialBlur`'s pass 1, `Hidden/CC_FastVignette`'s
// pass 0 and `Hidden/CC_Blend`'s pass 1. The game's texture coordinates run
// from the lower left: a place is taken so and turned to Bevy's (from the
// upper left) to sample.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct Effects {
    // The double vision's displacement in pixels and its amount.
    displace: vec2<f32>,
    amount: f32,
    // The wiggle's timer, scale and strength.
    timer: f32,
    scale: f32,
    strength: f32,
    // What the motion blur's picture keeps of the frame (`_AccumOrig`).
    keep: f32,
    double_on: f32,
    wiggle_on: f32,
    blur_on: f32,
    fresh: f32,
    // The flash's white (`_Coef`) and whether it is on.
    flash: f32,
    flash_on: f32,
    burn_on: f32,
    // Pain's blur's amount (`_Amount`), whether it is on, its centre
    // (`_Center`).
    radial: f32,
    radial_on: f32,
    radial_center: vec2<f32>,
    vignette_on: f32,
    // Near death's amount (`_Amount`).
    near_death: f32,
    // The vignette's `_Data`.
    vignette: vec4<f32>,
    near_death_on: f32,
}

@group(0) @binding(0) var picture: texture_2d<f32>;
@group(0) @binding(1) var linear_sampler: sampler;
@group(0) @binding(2) var<uniform> fx: Effects;
// Near death's picture (`_OverlayTex`), its pass's alone.
@group(0) @binding(3) var overlay: texture_2d<f32>;

// The picture at a place given from its lower left.
fn at(p: vec2<f32>) -> vec4<f32> {
    return textureSample(picture, linear_sampler, vec2<f32>(p.x, 1.0 - p.y));
}

@fragment
fn double_vision(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let uv = vec2<f32>(in.uv.x, 1.0 - in.uv.y);
    let d = fx.displace / vec2<f32>(textureDimensions(picture));
    let here = at(uv);
    var sum = at(uv + d * 8.0) * 0.5 + here;
    sum = at(uv + d * 16.0) * 0.3 + sum;
    sum = at(uv + d * 24.0) * 0.2 + sum;
    return fx.amount * (sum * 0.5 - here) + here;
}

// `Hidden/WhiteFlash`: toward white by the flash's coefficient.
@fragment
fn white_flash(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let c = textureSample(picture, linear_sampler, in.uv);
    return fx.flash * (vec4<f32>(1.0) - c) + c;
}

@fragment
fn wiggle(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let uv = (vec2<f32>(in.uv.x, 1.0 - in.uv.y) - 0.5) * 0.98 + 0.5;
    let x = sin(uv.x * fx.scale + fx.timer) * fx.strength * 0.01 + uv.x;
    let y = cos(uv.y * fx.scale + fx.timer) * fx.strength * 0.01 + uv.y;
    return at(vec2<f32>(x, y));
}

// Eight samples toward the centre, the i-th at `1 − amount × i / 7` of the
// way out, averaged.
@fragment
fn radial_blur(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let uv = vec2<f32>(in.uv.x, 1.0 - in.uv.y);
    let from_center = uv - fx.radial_center;
    var sum = vec4<f32>(0.0);
    for (var i = 0; i < 8; i = i + 1) {
        let scale = 1.0 - fx.radial * f32(i) * 0.142857;
        sum = sum + at(from_center * scale + fx.radial_center);
    }
    return sum * 0.125;
}

// Darkened toward the edges by the distance from the centre (in the
// screen's terms, 0 to 1 each way), smoothly; its alpha kept, all times
// `_Color` (white, its default: nothing sets it).
@fragment
fn vignette(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let uv = vec2<f32>(in.uv.x, 1.0 - in.uv.y);
    let d = length(uv - fx.vignette.xy);
    let t = clamp((d * (fx.vignette.z + fx.vignette.w) - 0.8) / (fx.vignette.z * 0.799 - 0.8), 0.0, 1.0);
    let keep = t * t * (3.0 - 2.0 * t);
    let c = textureSample(picture, linear_sampler, in.uv);
    return vec4<f32>(c.rgb * keep, c.a);
}

// The picture blended toward itself times the overlay by the amount (the
// overlay at the same place, its rows from the bottom as the game keeps
// them).
@fragment
fn near_death(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let c = textureSample(picture, linear_sampler, in.uv);
    let o = textureSample(overlay, linear_sampler, vec2<f32>(in.uv.x, 1.0 - in.uv.y));
    return fx.near_death * (c * o - c) + c;
}

@fragment
fn accumulate(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let frame = textureSample(picture, linear_sampler, in.uv);
    return vec4<f32>(frame.rgb, fx.keep);
}

@fragment
fn copy(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    return textureSample(picture, linear_sampler, in.uv);
}
