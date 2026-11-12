// (trp: the dithered glass, `Transparent/DepthZwriteDithered`'s deferred
// pass: `glass_view.rs`; body-animator spec, "The shield's glass".) Every
// figure is the game's number, a gamma project's: the colour picture read
// back to the figure it was stored as, the specular picture and the cube
// read as stored. Where the alpha is under a half every other pixel of a
// fixed checkerboard is thrown away and the colour doubled; each kept pixel
// is marked by an alpha of nought in the frame, which the camera's
// undithering (`undither.wgsl`) looks for.

#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing, calculate_tbn_mikktspace},
    mesh_view_bindings::{view, lights},
}
#ifdef TRP_GAME_SHADING
#import bevy_pbr::pbr_functions::{trp_game_light, TrpSurface}
#endif

struct Glass {
    // `_Color`, `_SpecColor`, `_ReflectColor`, as stored.
    color: vec4<f32>,
    spec_color: vec4<f32>,
    reflect_color: vec4<f32>,
    // `_OpacityScale`, `_SpecPower`, `_Glossness`, `_GlobalReflectionStrength`.
    figures: vec4<f32>,
    // The top ambient; w: 1 where it has a cube of its own.
    ambient: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> glass: Glass;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var spec_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var spec_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var cube_texture: texture_cube<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var cube_sampler: sampler;

const PI: f32 = 3.14159265;

// A linear colour as the stored figure it was read from (the sRGB curve,
// which the graphics card decodes a picture by), and back.
fn stored(c: vec3<f32>) -> vec3<f32> {
    let v = max(c, vec3(0.0));
    return select(1.055 * pow(v, vec3(1.0 / 2.4)) - 0.055, v * 12.92, v <= vec3(0.0031308));
}

fn linear(c: vec3<f32>) -> vec3<f32> {
    let v = max(c, vec3(0.0));
    return select(pow((v + 0.055) / 1.055, vec3(2.4)), v / 12.92, v <= vec3(0.04045));
}

// The checkerboard's thrown pixels: x + y odd, the pixel's place counted
// from the view's bottom left, as the game's screen position has it.
fn thrown(frag: vec2<f32>) -> bool {
    let p = vec2<i32>(floor(frag - view.viewport.xy));
    let y = i32(view.viewport.w) - 1 - p.y;
    return ((p.x + y) & 1) == 1;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    // The colour picture's texel (the material's colour is white here: the
    // shader takes `_Color` itself), as stored.
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let texel = pbr_input.material.base_color;
    let t = vec4(stored(texel.rgb), texel.a);
    let c = t * glass.color;
    let alpha = saturate(c.a * glass.figures.x);
#ifdef VERTEX_UVS_A
    let uv = in.uv;
#else
    let uv = vec2(0.0);
#endif
    let s = textureSample(spec_texture, spec_sampler, uv);
    let k = s.r * glass.figures.y;
    let g = s.a * glass.figures.z;

    // The surface's own frame: the way to the eye and the bumped normal in
    // it, as the game's vertex hands them to its fragment.
    let N0 = normalize(in.world_normal);
#ifdef VERTEX_TANGENTS
    let tbn = calculate_tbn_mikktspace(N0, in.world_tangent);
    let T = tbn[0];
    let B = tbn[1];
#else
    let T = normalize(cross(select(vec3(0.0, 1.0, 0.0), vec3(1.0, 0.0, 0.0), abs(N0.y) > 0.9), N0));
    let B = cross(N0, T);
#endif
    let N = pbr_input.N;
    let n_t = normalize(vec3(dot(N, T), dot(N, B), dot(N, N0)));
    let to_eye = view.world_position - in.world_position.xyz;
    let e = normalize(vec3(dot(to_eye, T), dot(to_eye, B), dot(to_eye, N0)));
    // The rim: 0.3 face on, 1 edge on.
    let rim = 0.3 + 0.7 * pow(1.0 - dot(normalize(abs(e)), n_t), 3.0);
    // The dither's gain.
    let gain = select(1.0, 2.0, alpha <= 0.5);
    let albedo = c.rgb * gain;
    // The material's cube, looked up in the surface's own frame (it turns
    // with the glass); with none of its own, the engine's default cube, a
    // flat 0x80. The level's reflection probe (`_MyGlobalReflectionProbe`)
    // is not in the app: nought.
    let cube = textureSample(cube_texture, cube_sampler, reflect(e, n_t)).rgb;
    let r_c = select(vec3(128.0 / 255.0), cube, glass.ambient.w > 0.5) * glass.reflect_color.rgb;
    let r_p = vec3(0.0);
    let E = 2.0 * s.r * (t.rgb * glass.color.rgb + glass.figures.w * r_p + r_c) * glass.ambient.rgb;
    let emission = E * rim * gain;
    let specular = saturate((g + alpha * (1.0 - rim)) * glass.spec_color.rgb / PI);
    let smoothness = saturate(k);

    if alpha < 0.5 && thrown(in.position.xy) {
        discard;
    }
    pbr_input.material.perceptual_roughness = 1.0 - smoothness;
    pbr_input.material.metallic = 0.0;
    var out: FragmentOutput;
#ifdef TRP_GAME_SHADING
    if lights.ambient_color.a < 0.0 {
        // The game's light on the G-buffer's colour (held to one), specular
        // and smoothness, with the emission and the glass's own ambient.
        var surface: TrpSurface;
        surface.color = saturate(albedo);
        surface.specular = specular;
        surface.smoothness = smoothness;
        surface.N = N;
        surface.V = pbr_input.V;
        out.color = main_pass_post_lighting_processing(pbr_input, vec4(trp_game_light(pbr_input, surface, emission, albedo), 1.0));
        out.color.a = 0.0;
        return out;
    }
#endif
    // Bevy's light (`TARKOV SHADING` off, and the views that keep it): the
    // figures made linear, the specular colour as the reflectance's.
    pbr_input.material.base_color = vec4(linear(saturate(albedo)), 1.0);
    pbr_input.material.reflectance = sqrt(linear(specular) / 0.16);
    pbr_input.material.emissive = vec4(linear(emission), 0.0);
    out.color = main_pass_post_lighting_processing(pbr_input, apply_pbr_lighting(pbr_input));
    // The mark.
    out.color.a = 0.0;
    return out;
}
