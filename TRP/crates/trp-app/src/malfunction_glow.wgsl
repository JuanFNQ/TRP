// The malfunction glow's picture added onto the screen (`wear.rs`; the
// game's `UI/Additive`: Blend SrcAlpha One, its colour twice the vertex
// colour times `_TintColor` of a half times the picture's).

#import bevy_ui::ui_vertex_output::UiVertexOutput

@group(1) @binding(0) var<uniform> alpha: vec4<f32>;
@group(1) @binding(1) var<uniform> cut: vec4<f32>;
@group(1) @binding(2) var picture: texture_2d<f32>;
@group(1) @binding(3) var picture_sampler: sampler;

// The sRGB curve: a colour's numbers as the game's screens have them, where
// those are composed in gamma space (`gamma_ui.rs`).
fn gamma_numbers(c: vec3<f32>) -> vec3<f32> {
    let v = max(c, vec3(0.0));
    return select(1.055 * pow(v, vec3(1.0 / 2.4)) - 0.055, v * 12.92, v <= vec3(0.0031308));
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    // The cut's top is its highest row: the texture is kept as the engine
    // has it, its first row lowest.
    let size = vec2<f32>(textureDimensions(picture));
    let texel = vec2(mix(cut.x, cut.z, in.uv.x), mix(cut.w, cut.y, in.uv.y));
    let p = textureSample(picture, picture_sampler, texel / size);
    var rgb = p.rgb;
    if (alpha.y > 0.5) {
        rgb = gamma_numbers(rgb);
    }
    // Added as it is: what the blend adds is the colour times the picture's
    // alpha times the glow's.
    return vec4(rgb * p.a * alpha.x, 0.0);
}
