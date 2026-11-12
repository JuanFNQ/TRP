// The inspect window's preview's photographic tonemap (`preview_tonemap.rs`):
// Standard Assets' `Tonemapping` of the type 3, `1 - 2^(-exposure c)`, on
// the frame's figures as the game's numbers (the 2.2 power) and back; the
// alpha, the picture's cover, kept. Or the OVERALL figure's camera's PRISM:
// its filmic curve squared (as `prism.wgsl`'s main pass, with no exposure),
// on the frame held to nought and one (an eight-bit picture's) and taken
// into the game's numbers. Either way the result is the number the picture
// is to store: written as the light the picture's sRGB encoding turns back
// into that number (until 0.249.0 by the 2.2 power, which the encoding
// does not undo: a stored 0.06 came out 0.027).

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct Tonemap {
    // The exposure (x), the curve (y: 0 photographic, 1 filmic).
    figures: vec4<f32>,
    // The filmic's tone and secondary tone.
    tone: vec4<f32>,
    secondary: vec4<f32>,
}

@group(0) @binding(0) var picture: texture_2d<f32>;
@group(0) @binding(1) var picture_sampler: sampler;
@group(0) @binding(2) var<uniform> tonemap: Tonemap;

// The light whose sRGB encoding is `stored` (held to nought and one).
fn to_store(stored: vec3<f32>) -> vec3<f32> {
    let v = clamp(stored, vec3<f32>(0.0), vec3<f32>(1.0));
    return select(pow((v + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4)), v / 12.92, v <= vec3<f32>(0.04045));
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let c = textureSample(picture, picture_sampler, in.uv);
    if (tonemap.figures.y > 0.5) {
        let numbers = pow(clamp(c.rgb, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(1.0 / 2.2));
        let x = max(numbers - vec3<f32>(tonemap.secondary.x), vec3<f32>(0.0));
        let curve = x * (x * tonemap.tone.x + tonemap.tone.y) / (x * (x * tonemap.tone.x + tonemap.tone.z) + tonemap.secondary.y);
        return vec4<f32>(to_store(curve * curve), c.a);
    }
    let numbers = pow(max(c.rgb, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.2));
    let mapped = vec3<f32>(1.0) - exp2(-tonemap.figures.x * numbers);
    return vec4<f32>(to_store(mapped), c.a);
}
