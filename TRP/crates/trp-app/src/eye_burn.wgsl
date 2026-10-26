// PROVISIONAL: the spot's picture has its lower left at its square's (the
// engine's flip of its drawing not read), and `grayscaleRamp` is taken as the
// brightness itself.
// The eyes' burn (`contusion_view.rs`; grenade spec, "A flash"), as the
// game's compiled shaders do it: each spot as `Custom/Eye Burn` draws it
// (its picture turned by `_Rotation` about its middle, its colour taken
// from the grey ramp by its brightness and blended toward its own by its
// life left, `_Coef`, all times that), the spots combined by their largest
// (the shader's blend), then laid over the frame by a screen blend
// (`Hidden/EyeBurnScreenEffect`). A spot's place and size are in the
// screen's terms, from −1 to 1 each way with y up.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct Burn {
    // Two rows a spot: its middle and half size; its turn, life left and
    // picture.
    spots: array<vec4<f32>, 64>,
    count: f32,
    _pad: vec3<f32>,
}

@group(0) @binding(0) var picture: texture_2d<f32>;
@group(0) @binding(1) var linear_sampler: sampler;
@group(0) @binding(2) var<uniform> burn: Burn;
@group(0) @binding(3) var spot0: texture_2d<f32>;
@group(0) @binding(4) var spot1: texture_2d<f32>;
@group(0) @binding(5) var spot2: texture_2d<f32>;
@group(0) @binding(6) var spot3: texture_2d<f32>;
@group(0) @binding(7) var spot_sampler: sampler;

fn spot_picture(which: i32, uv: vec2<f32>, level: f32) -> vec4<f32> {
    switch which {
        case 0: { return textureSampleLevel(spot0, spot_sampler, uv, level); }
        case 1: { return textureSampleLevel(spot1, spot_sampler, uv, level); }
        case 2: { return textureSampleLevel(spot2, spot_sampler, uv, level); }
        default: { return textureSampleLevel(spot3, spot_sampler, uv, level); }
    }
}

@fragment
fn eye_burn(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let frame = textureSample(picture, linear_sampler, in.uv);
    let size = vec2<f32>(textureDimensions(picture));
    // The pixel in the screen's terms, y up.
    let p = vec2<f32>(in.uv.x * 2.0 - 1.0, 1.0 - in.uv.y * 2.0);
    var lit = vec4<f32>(0.0);
    for (var i = 0; i < i32(burn.count); i = i + 1) {
        let a = burn.spots[2 * i];
        let b = burn.spots[2 * i + 1];
        let half = a.zw;
        if (half.x <= 0.0 || half.y <= 0.0) {
            continue;
        }
        // Its own place in its square, from its lower left.
        let local = (p - (a.xy - half)) / (2.0 * half);
        if (any(local < vec2<f32>(0.0)) || any(local > vec2<f32>(1.0))) {
            continue;
        }
        // Turned as the shader's vertices turn it.
        let r = local - 0.5;
        let s = sin(b.x);
        let c = cos(b.x);
        let uv = vec2<f32>(c * r.x + s * r.y, -s * r.x + c * r.y) + 0.5;
        let flipped = vec2<f32>(uv.x, 1.0 - uv.y);
        // The level its size on the screen asks of its picture.
        let pixels = max(half.x * size.x, 1.0);
        let level = max(log2(f32(textureDimensions(spot0).x) / pixels), 0.0);
        let own = spot_picture(i32(b.z), flipped, level);
        let grey = dot(own.rgb, vec3<f32>(0.22, 0.707, 0.071));
        let coef = b.y;
        let colour = vec3<f32>(grey) * (1.0 - coef) + own.rgb * coef;
        lit = max(lit, vec4<f32>(colour, own.a) * coef);
    }
    return vec4<f32>(1.0) - (vec4<f32>(1.0) - frame) * (vec4<f32>(1.0) - lit);
}
