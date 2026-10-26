// The sky's scattering over the scene (`scattering_view.rs`; lighting spec,
// "The sky's scattering over the scene"): `Hidden/Time of Day/Scattering`'s
// pass 0 (its HDR, gamma variant without `LIGHTEN`), on the frame in the
// game's numbers before the world's effects, as the game's opaque image
// effect runs after the opaque scene. The share by the distance to the
// scene and its height over the level's zero; the colour the sky's
// scattering along the way, shortened by the sunrise glow and worked out
// at each pixel; where the scene is the sky, only below the horizon. The
// game's `_StencilShadow` (its indoor volumes) is nought on the range.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct Scattering {
    // The sky's figures, as `tod_sky.wgsl`'s.
    sun_sky: vec4<f32>,
    moon_sky: vec4<f32>,
    ground: vec4<f32>,
    moon_halo: vec4<f32>,
    sun: vec4<f32>,
    moon: vec4<f32>,
    // `TOD_Contrast`, `TOD_ScatteringBrightness`, `TOD_Fogginess`,
    // `TOD_MoonHaloPower`.
    figures: vec4<f32>,
    k_beta_mie: vec4<f32>,
    k_sun: vec4<f32>,
    k_4pi: vec4<f32>,
    k_radius: vec4<f32>,
    k_scale: vec4<f32>,
    // `_Density`: the height falloff, the camera's height over the zero
    // level, the density; and `_SunrizeGlow`.
    density: vec4<f32>,
    // The world from the clip, and the camera's place (the app's axes).
    world_from_clip: mat4x4<f32>,
    camera: vec4<f32>,
    // `_DitheringTexture`'s 8 by 8 alphas, its rows as stored (the first at
    // the bottom), four to a vector.
    dithering: array<vec4<f32>, 16>,
}

@group(0) @binding(0) var picture: texture_2d<f32>;
@group(0) @binding(1) var depth: texture_2d<f32>;
@group(0) @binding(2) var<uniform> s: Scattering;

fn scale_exp(cos_angle: f32) -> f32 {
    let n = 1.0 - cos_angle;
    return exp(-0.00287 + n * (0.459 + n * (3.83 + n * (-6.8 + n * 5.25))));
}

// The share of the light scattered in over a way through the air, thinning
// with the height over the zero level.
fn share(offset: vec3<f32>) -> f32 {
    let thin = 1.0 / (1.0 + max((offset.y + s.density.y) * s.density.x, 0.0));
    return 1.0 - exp(-min(length(offset) * thin * max(s.density.z, 0.001), 10.0));
}

// A place in the world at a depth of this pixel.
fn world_at(uv: vec2<f32>, d: f32) -> vec3<f32> {
    let p = s.world_from_clip * vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, d, 1.0);
    return p.xyz / p.w;
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(in.position.xy);
    let source = textureLoad(picture, pixel, 0);
    let d = textureLoad(depth, pixel, 0).r;
    // The way from the eye, in the sky's axes (the game's: z turned round).
    let toward = normalize(world_at(in.uv, 1.0) - s.camera.xyz);
    let way = vec3<f32>(toward.x, toward.y, -toward.z);
    // The scene's share (nought where it is the sky, at the far plane: depth
    // nought).
    var amount = 0.0;
    if (d > 0.0) {
        amount = share(world_at(in.uv, d) - s.camera.xyz);
    }
    if (amount == 0.0) {
        let t = clamp((0.005 - way.y) * 200.0, 0.0, 1.0);
        amount = t * t * (3.0 - 2.0 * t);
        if (amount <= 0.0) {
            return source;
        }
    }

    // The sky's scattering along the way shortened by the sunrise glow, its
    // height bent near the horizon, four samples, each weighed by the share.
    let v = way * s.density.w;
    var yb = select((v.y + 0.03125) * (v.y + 0.03125) * 8.0, v.y, v.y > 0.03125);
    yb = select(0.0, yb, v.y > -0.03125);
    let dir = vec3<f32>(v.x, yb, v.z);
    let x = s.k_radius.x;
    let start_height = x + s.k_scale.w;
    let far = sqrt(yb * yb * s.k_radius.y + s.k_radius.w - s.k_radius.y) - x * yb;
    let start_offset = exp(-s.k_scale.w * s.k_scale.z) * scale_exp(yb);
    let sample_length = far * 0.25;
    let scaled_length = amount * s.k_scale.x * sample_length;
    var at = vec3<f32>(0.0, start_height, 0.0) + dir * sample_length * 0.5;
    let extinction = s.k_4pi.w + s.k_4pi.xyz;
    var gathered = vec3<f32>(0.0);
    for (var i = 0; i < 4; i++) {
        let height = length(at);
        let inv = 1.0 / height;
        let thick = exp((x - height) * s.k_scale.z);
        let camera_scale = scale_exp(dot(dir, at) * inv) * 0.25;
        let light_scale = scale_exp(dot(s.sun.xyz, at) * inv);
        let scatter = start_offset * 0.25 + thick * (light_scale * 0.25 - camera_scale);
        gathered += exp(-scatter * extinction) * scaled_length * thick;
        at += dir * sample_length;
    }
    let lit = gathered * s.sun_sky.rgb;
    let rayleigh = lit * s.k_sun.xyz;
    let mie = lit * s.k_sun.w;
    let c_sun = dot(s.sun.xyz, v);
    let mie_phase = s.k_beta_mie.x * (1.0 + c_sun * c_sun) / pow(s.k_beta_mie.z * c_sun + s.k_beta_mie.y, 1.5);
    var c = mie * mie_phase + (0.75 + 0.75 * c_sun * c_sun) * rayleigh;
    c += s.moon_sky.rgb - v.y * s.moon_sky.rgb;
    c += s.moon_halo.rgb * pow(max(dot(v, s.moon.xyz), 0.0), s.figures.w);
    let grey = (c.r + c.g + c.b) * 0.333;
    c += s.figures.z * (vec3<f32>(grey) - c);
    let below = clamp(v.y * -0.8, 0.0, 1.0);
    c += below * below * (3.0 - 2.0 * below) * (s.ground.rgb - c);
    // The dithering's alpha at this pixel, rows counted from the bottom.
    let size = vec2<i32>(textureDimensions(picture));
    let cell = vec2<i32>(pixel.x % 8, (size.y - 1 - pixel.y) % 8);
    let index = cell.y * 8 + cell.x;
    let dither = s.dithering[index / 4][index % 4];
    let colour = sqrt(pow(c * s.figures.y, vec3<f32>(s.figures.x)) + vec3<f32>(dither * 0.015385));
    return vec4<f32>(source.rgb + amount * (colour - source.rgb), source.a);
}
