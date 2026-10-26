// A sprite's cut filled radially from the bottom, clockwise (`radial_fill.rs`;
// the engine's `Image.Type.Filled`, `Radial360`, its origin `Bottom`).

#import bevy_ui::ui_vertex_output::UiVertexOutput

@group(1) @binding(0) var<uniform> color: vec4<f32>;
@group(1) @binding(1) var<uniform> fill: vec4<f32>;
@group(1) @binding(2) var<uniform> cut: vec4<f32>;
@group(1) @binding(3) var sprite: texture_2d<f32>;
@group(1) @binding(4) var sprite_sampler: sampler;

// The sRGB curve: a colour's numbers as the game's screens have them, where
// those are composed in gamma space (`gamma_ui.rs`).
fn gamma_numbers(c: vec4<f32>) -> vec4<f32> {
    let v = max(c.rgb, vec3(0.0));
    let encoded = select(1.055 * pow(v, vec3(1.0 / 2.4)) - 0.055, v * 12.92, v <= vec3(0.0031308));
    return vec4(encoded, c.a);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    // The turn about the middle from straight down, clockwise on the
    // screen (the node's v grows downwards), as a share of a whole turn.
    let p = in.uv - vec2(0.5);
    var turn = atan2(-p.x, p.y) / 6.2831853;
    if (turn < 0.0) {
        turn += 1.0;
    }
    if (turn > fill.x) {
        discard;
    }
    // The picture turned clockwise about the node's middle by `fill.z` of a
    // whole turn (a spinner): each point drawn from the picture's point that
    // far back, worked in pixels so that a node not square keeps its shape;
    // what falls outside the picture is left clear.
    var uv = in.uv;
    if (fill.z != 0.0) {
        let angle = fill.z * 6.2831853;
        let c = cos(angle);
        let s = sin(angle);
        let q = (in.uv - vec2(0.5)) * in.size;
        uv = vec2(c * q.x + s * q.y, -s * q.x + c * q.y) / in.size + vec2(0.5);
        if (any(uv < vec2(0.0)) || any(uv > vec2(1.0))) {
            discard;
        }
    }
    // The cut's top is its highest row: the texture is kept as the engine
    // has it, its first row lowest.
    let size = vec2<f32>(textureDimensions(sprite));
    let texel = vec2(mix(cut.x, cut.z, uv.x), mix(cut.w, cut.y, uv.y));
    let picture = textureSample(sprite, sprite_sampler, texel / size);
    if (fill.y > 0.5) {
        return gamma_numbers(color) * gamma_numbers(picture);
    }
    return color * picture;
}
