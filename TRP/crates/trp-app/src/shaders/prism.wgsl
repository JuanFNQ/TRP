// PRISM's exposure, vignette and filmic tonemap (`prism_view.rs`; lighting
// spec, "PRISM"), on the frame in the game's gamma-encoded numbers: the
// brightness taken from the frame (the secondary shader's pass 3), halved
// down to a texel (pass 2), blended into the kept one (pass 4), and the
// main pass (`Hidden/PrismEffects`, the variant with its filmic tonemap and
// exposure on).

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct Prism {
    // The middle grey, the lower and upper limits, the adaptation's share
    // this frame.
    exposure: vec4<f32>,
    // `_ToneParams` and `_SecondaryToneParams`.
    tone: vec4<f32>,
    secondary: vec4<f32>,
    // `_VignetteStart`, `_VignetteEnd`, `_VignetteIntensity`, on.
    vignette: vec4<f32>,
    vignette_color: vec4<f32>,
}

@group(0) @binding(0) var picture: texture_2d<f32>;
@group(0) @binding(1) var linear_sampler: sampler;
@group(0) @binding(2) var<uniform> prism: Prism;

// Pass 3: one bilinear sample of the frame, held between nought and the
// half float's top, weighed to a brightness in every channel.
@fragment
fn brightness_first(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let c = clamp(textureSample(picture, linear_sampler, in.uv).rgb, vec3(0.0), vec3(65472.0));
    return vec4(dot(c, vec3(0.22, 0.707, 0.071)));
}

// Pass 2: the mean of four bilinear samples one texel of the picture off
// along the diagonals.
@fragment
fn halve(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let texel = 1.0 / vec2<f32>(textureDimensions(picture));
    let sum = textureSample(picture, linear_sampler, in.uv + texel * vec2(-1.0, 1.0)).rgb
        + textureSample(picture, linear_sampler, in.uv + texel).rgb
        + textureSample(picture, linear_sampler, in.uv - texel).rgb
        + textureSample(picture, linear_sampler, in.uv + texel * vec2(1.0, -1.0)).rgb;
    return vec4(sum * 0.25, 1.0);
}

// Pass 4: the frame's brightness, with the share it is blended in by
// (source alpha over one minus it).
@fragment
fn adapt(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    return vec4(vec3(textureSample(picture, linear_sampler, in.uv).r), prism.exposure.w);
}

@group(1) @binding(0) var kept: texture_2d<f32>;

// The main pass: the exposure, the vignette, the filmic curve squared.
@fragment
fn main_pass(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let c = textureSample(picture, linear_sampler, in.uv);
    let brightness = textureSample(kept, linear_sampler, vec2(0.5)).r;
    let grey = prism.exposure.x;
    // Over a kept brightness of nought the game's sum is endless, held to
    // the upper limit.
    let multiplier = select(prism.exposure.z, max(min(grey / brightness, prism.exposure.z), prism.exposure.y), brightness > 0.0);
    var rgb = c.rgb * multiplier;
    if (prism.vignette.w > 0.0) {
        let d = length(in.uv - vec2(0.5));
        let t = clamp((d * prism.vignette.z - prism.vignette.x) * (1.0 / (prism.vignette.y - prism.vignette.x)), 0.0, 1.0);
        let s = t * t * (t * -2.0 + 3.0);
        let toward = rgb * prism.vignette_color.rgb;
        rgb = s * (rgb - toward) + toward;
    }
    let x = max(rgb - vec3(prism.secondary.x), vec3(0.0));
    let curve = x * (x * prism.tone.x + prism.tone.y) / (x * (x * prism.tone.x + prism.tone.z) + prism.secondary.y);
    return vec4(curve * curve, c.a);
}
