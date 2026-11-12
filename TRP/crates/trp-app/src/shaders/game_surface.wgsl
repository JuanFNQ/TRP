// (trp: the game's own surface, `p0/Reflective/Bumped Specular SMap`, for an
// item's picture, the inspect window's preview and the OVERALL tab's own
// figure: `game_surface.rs`, `figure_surface.rs`; icons spec, "The game's
// surface in a picture"; inspect-window spec, "The model"; inventory spec,
// "The character's surface"; and the dithered glass's in each, `Transparent/
// DepthZwriteDithered_Icon` and the world's deferred pass.) Every figure is
// the game's number, a gamma project's: a picture is read back to the
// figure it was stored as, and the sum handed on as the view's target
// takes it.

#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, calculate_tbn_mikktspace},
    mesh_view_bindings::view,
}
// (The OVERALL figure's: the game's light, which the patched PBR functions
// carry, `shading.rs`.)
#ifdef TRP_GAME_SHADING
#import bevy_pbr::pbr_functions::{trp_game_light, TrpSurface}
#endif

struct SurfaceFigures {
    color: vec4<f32>,
    spec_color: vec4<f32>,
    reflect_color: vec4<f32>,
    // _DefVals.xy, _SpecVals.xy; the glass's _OpacityScale, _SpecPower,
    // _Glossness, _GlobalReflectionStrength
    vals: vec4<f32>,
    // _Specularness, _Glossness, has a gloss picture, has a cube; the
    // glass's top ambient, has a cube
    shine: vec4<f32>,
    // the eye where w is 1
    eye: vec4<f32>,
    // x: 0 a picture, 1 the preview, 2 the app's own in a picture, 3 a
    // picture's glass, 4 the preview's glass, 5 a picture's glass's alpha;
    // y: the lights; z: the colour read raw; w: a picture's glass's base
    // pass's light
    way: vec4<f32>,
    light_to: array<vec4<f32>, 4>,
    light_color: array<vec4<f32>, 4>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> surface: SurfaceFigures;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var surface_cube: texture_cube<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var surface_cube_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var surface_spec: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var surface_spec_sampler: sampler;

const PI: f32 = 3.14159265;

// A linear colour as the stored figure it was read from (the sRGB curve,
// which the graphics card decodes a picture by).
fn stored(c: vec3<f32>) -> vec3<f32> {
    let v = max(c, vec3(0.0));
    return select(1.055 * pow(v, vec3(1.0 / 2.4)) - 0.055, v * 12.92, v <= vec3(0.0031308));
}


fn pow5(x: f32) -> f32 {
    let x2 = x * x;
    return x2 * x2 * x;
}

// The glass's checkerboard's thrown pixels (`glass.wgsl`): x + y odd, the
// pixel's place counted from the view's bottom left.
fn thrown(frag: vec2<f32>) -> bool {
    let p = vec2<i32>(floor(frag - view.viewport.xy));
    let y = i32(view.viewport.w) - 1 - p.y;
    return ((p.x + y) & 1) == 1;
}

// The cube at a way in the engine's world: the install's cube is in the
// game's, whose z runs the other way. With none of its own, the engine's
// default cube, a flat 0x80.
fn cube_at(R: vec3<f32>) -> vec3<f32> {
    if surface.shine.w < 0.5 {
        return vec3(128.0 / 255.0);
    }
    return textureSample(surface_cube, surface_cube_sampler, vec3(R.x, R.y, -R.z)).rgb;
}

// The deferred light (the game's copy of `Hidden/Internal-DeferredShading`, as
// `game_shading.wgsl`'s `trp_standard_light` and `trp_sim::shading::
// standard_light` have it), one light: c the colour, s the specular, g the
// smoothness, `light` its colour times intensity.
fn deferred_light(c: vec3<f32>, s: vec3<f32>, g: f32, N: vec3<f32>, V: vec3<f32>, L: vec3<f32>, light: vec3<f32>) -> vec3<f32> {
    let sum = L + V;
    let H = sum * inverseSqrt(max(dot(sum, sum), 0.001));
    let nl = saturate(dot(N, L));
    let nh = saturate(dot(N, H));
    let lh = saturate(dot(L, H));
    let nv = abs(dot(N, V));
    let perceptual = 1.0 - g;
    let a = max(perceptual * perceptual, 0.002);
    let a2 = a * a;
    let grazing = 2.0 * lh * lh * perceptual - 0.5;
    let diffuse = (1.0 + grazing * pow5(1.0 - nl)) * (1.0 + grazing * pow5(1.0 - nv)) * nl;
    let visibility = 0.5 / (nl * (nv * (1.0 - a) + a) + nv * (nl * (1.0 - a) + a) + 1e-5);
    let d = (nh * a2 - nh) * nh + 1.0;
    let distribution = a2 * 0.31830988 / (d * d + 1e-7);
    var specular = sqrt(max(visibility * distribution * 3.14159265, 1e-4)) * nl;
    if all(s == vec3(0.0)) {
        specular = 0.0;
    }
    let fresnel = s + (1.0 - s) * pow5(1.0 - lh);
    return c * light * diffuse + specular * light * fresnel;
}

// The preview's deferred pass's G-buffer: its colour, specular and
// smoothness held to eight bits, its emission the cube's reflection (the
// ambient black). m the colour picture's texel as stored, gloss the gloss
// picture's, c the way to the eye against the normal, R the reflection.
struct Deferred {
    albedo: vec3<f32>,
    specular: vec3<f32>,
    smoothness: f32,
    emission: vec3<f32>,
}

fn deferred_surface(m: vec4<f32>, gloss: f32, c: f32, R: vec3<f32>) -> Deferred {
    let f = 0.5 * (1.0 - c) * (1.0 - c);
    let ks = 0.5 * (surface.vals.z + surface.vals.w * f);
    let kd = surface.vals.x + surface.vals.y * f;
    var out: Deferred;
    out.albedo = saturate(m.rgb * surface.color.rgb * kd);
    out.specular = saturate(m.a * surface.shine.y * ks * surface.spec_color.rgb);
    out.smoothness = saturate(gloss * surface.shine.x);
    out.emission = cube_at(R) * m.a * surface.reflect_color.rgb * ks;
    return out;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    let pbr_input = pbr_input_from_standard_material(in, is_front);
    // (Cut where the material is cut; opaque, nothing.)
    _ = alpha_discard(pbr_input.material, pbr_input.material.base_color);
    // The colour picture's texel as stored, its alpha the mask of the
    // specular and the reflection; the gloss picture's red (its roughness
    // here, one less it), white where there is none.
    let texel = pbr_input.material.base_color;
    let m = vec4(select(stored(texel.rgb), texel.rgb, surface.way.z > 0.5), texel.a);
    let gloss = select(1.0, 1.0 - pbr_input.material.perceptual_roughness, surface.shine.z > 0.5);
    let n = normalize(pbr_input.N);
    let P = in.world_position.xyz;
    let eye = select(view.world_position, surface.eye.xyz, surface.eye.w > 0.5);
    let V = normalize(eye - P);
    let c = dot(V, n);
    let R = reflect(-V, n);
    let lights = u32(surface.way.y);
#ifdef VERTEX_UVS_A
    let uv = in.uv;
#else
    let uv = vec2(0.0);
#endif
    var out: FragmentOutput;
    if surface.way.x > 5.5 {
        // The OVERALL figure (`figure_surface`): the preview's G-buffer, lit
        // by the view's own lights (handed in the game's units) through the
        // game's light rule: each light by the engine's attenuation, the
        // emission, no ambient (the view's is black), linear by the 2.2
        // power as the frame's figures (the preview's tonemap takes it
        // back).
        let g = deferred_surface(m, gloss, c, R);
#ifdef TRP_GAME_SHADING
        var s: TrpSurface;
        s.color = g.albedo;
        s.specular = g.specular;
        s.smoothness = g.smoothness;
        s.N = n;
        s.V = V;
        out.color = vec4(trp_game_light(pbr_input, s, g.emission, vec3(0.0)), 1.0);
#else
        // (Not drawn so: made only once the PBR functions are patched.)
        out.color = vec4(pow(max(g.emission, vec3(0.0)), vec3(2.2)), 1.0);
#endif
    } else if surface.way.x > 4.5 {
        // A picture's glass's alpha (`_Icon`'s pass 0, its alpha alone
        // written): its vertex colour's, one where it has none.
#ifdef VERTEX_COLORS
        out.color = vec4(0.0, 0.0, 0.0, saturate(in.color.a));
#else
        out.color = vec4(0.0, 0.0, 0.0, 1.0);
#endif
    } else if surface.way.x > 3.5 {
        // The preview's glass: the world's deferred pass (`glass.wgsl`),
        // its G-buffer lit by the preview's lights, the ambient black.
        let c = m * surface.color;
        let alpha = saturate(c.a * surface.vals.x);
        let s = textureSample(surface_spec, surface_spec_sampler, uv);
        let k = s.r * surface.vals.y;
        let g = s.a * surface.vals.z;
        // The surface's own frame: the way to the eye and the bumped normal
        // in it.
        let N0 = normalize(in.world_normal);
#ifdef VERTEX_TANGENTS
        let tbn = calculate_tbn_mikktspace(N0, in.world_tangent);
        let T = tbn[0];
        let B = tbn[1];
#else
        let T = normalize(cross(select(vec3(0.0, 1.0, 0.0), vec3(1.0, 0.0, 0.0), abs(N0.y) > 0.9), N0));
        let B = cross(N0, T);
#endif
        let n_t = normalize(vec3(dot(n, T), dot(n, B), dot(n, N0)));
        let to_eye = eye - P;
        let e = normalize(vec3(dot(to_eye, T), dot(to_eye, B), dot(to_eye, N0)));
        // The rim, 0.3 face on, 1 edge on; the dither's gain.
        let rim = 0.3 + 0.7 * pow(max(1.0 - dot(normalize(abs(e)), n_t), 0.0), 3.0);
        let gain = select(1.0, 2.0, alpha <= 0.5);
        let albedo = saturate(c.rgb * gain);
        // The cube in the surface's own frame; with none, the engine's
        // default; the level's probe not in the app.
        let cube = textureSample(surface_cube, surface_cube_sampler, reflect(e, n_t)).rgb;
        let r_c = select(vec3(128.0 / 255.0), cube, surface.shine.w > 0.5) * surface.reflect_color.rgb;
        let E = 2.0 * s.r * (m.rgb * surface.color.rgb + r_c) * surface.shine.rgb;
        let specular = saturate((g + alpha * (1.0 - rim)) * surface.spec_color.rgb / PI);
        let smoothness = saturate(k);
        if alpha < 0.5 && thrown(in.position.xy) {
            discard;
        }
        var sum = E * rim * gain;
        for (var j = 0u; j < lights; j = j + 1u) {
            sum += deferred_light(albedo, specular, smoothness, n, V, surface.light_to[j].xyz, surface.light_color[j].rgb);
        }
        // In the frame's figures, its alpha nought: the mark the camera's
        // undithering looks for.
        out.color = vec4(pow(max(sum, vec3(0.0)), vec3(2.2)), 0.0);
    } else if surface.way.x > 2.5 {
        // A picture's glass: `_Icon`'s passes 1 (the base light and the
        // reflection) and 2 (each other light), each sum held to one as
        // the eight-bit target holds it, blended by the colour's alpha over
        // what is behind (the pipeline's blend, the colour alone written).
        // (No glass of the install is tinted: `_Tinted`'s 0.05 is not here.)
        let c = m * surface.color;
        let s = textureSample(surface_spec, surface_spec_sampler, uv);
        let k = s.r * surface.vals.y;
        let g = s.a * surface.vals.z;
        let refl = 2.5 * cube_at(R) * surface.reflect_color.rgb + s.r * mix(vec3(1.0), c.rgb, c.a) * vec3(0.254, 0.310, 0.388);
        let base = u32(surface.way.w);
        var sum = select(vec3(0.0), min(refl, vec3(1.0)), lights == 0u);
        for (var j = 0u; j < lights; j = j + 1u) {
            let L = surface.light_to[j].xyz;
            let C = surface.light_color[j].rgb;
            let H = normalize(V + L);
            let nh = max(dot(n, H), 1e-6);
            let lit = c.rgb * C * max(dot(n, L), 0.0) + C * surface.spec_color.rgb * pow(nh, 128.0 * k) * g;
            sum += min(select(lit, lit + refl, j == base), vec3(1.0));
        }
        out.color = vec4(sum, c.a);
    } else if surface.way.x > 1.5 {
        // The app's own material in a picture: the standard surface's light,
        // as the stored figure an sRGB picture would have kept of it.
        let lit = apply_pbr_lighting(pbr_input);
        out.color = vec4(stored(lit.rgb), 1.0);
    } else if surface.way.x < 0.5 {
        // A picture: `_Icon`'s ForwardBase and one ForwardAdd a light.
        let f = (1.0 - c) * (1.0 - c);
        let d = surface.vals.x + surface.vals.y * f;
        let s = surface.vals.z + surface.vals.w * f;
        let albedo = m.rgb * surface.color.rgb * d;
        let exponent = 128.0 * gloss * surface.shine.x;
        let mask = m.a * surface.shine.y * s;
        var sum = m.a * surface.reflect_color.rgb * cube_at(R) * s;
        for (var k = 0u; k < lights; k = k + 1u) {
            let L = surface.light_to[k].xyz;
            let C = surface.light_color[k].rgb;
            let H = normalize(V + L);
            let nh = max(dot(n, H), 1e-6);
            sum += albedo * C * max(dot(n, L), 0.0) + C * surface.spec_color.rgb * pow(nh, exponent) * mask;
        }
        // Into an eight-bit picture as the figures themselves (the studio's
        // camera hands them on as they are), alpha whole.
        out.color = vec4(min(sum, vec3(1.0)), 1.0);
    } else {
        // The preview: the deferred pass's G-buffer, then each light.
        let g = deferred_surface(m, gloss, c, R);
        var sum = g.emission;
        for (var k = 0u; k < lights; k = k + 1u) {
            sum += deferred_light(g.albedo, g.specular, g.smoothness, n, V, surface.light_to[k].xyz, surface.light_color[k].rgb);
        }
        // In half floats, as the frame's figures (the 2.2 power the
        // preview's tonemap takes back).
        out.color = vec4(pow(max(sum, vec3(0.0)), vec3(2.2)), 1.0);
    }
    return out;
}
