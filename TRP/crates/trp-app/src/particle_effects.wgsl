// The particle effects' systems (`explosion_view`; grenades spec, "The
// blast's effect"), each drawn by a port of its material's game shader
// (each disassembled, the variant with soft particles and without the
// windows' MBOIT). The corners are worked out before (billboards and
// stretched billboards alike), in the app's axes: their place, their
// picture's place (the sheet's cell already), their colour, and the normal
// facing the camera. On the frame in its gamma-encoded numbers, as the game
// blends (`gamma_ui`). The level's light is Factory by day's with `FACTORY
// LIGHT` (`explosion_view::effect_light`); the sky's scattering fog is
// none: each fragment says.

struct Effect {
    clip_from_world: mat4x4<f32>,
    view_from_world: mat4x4<f32>,
    // The camera's near plane, `_Time.x`, `_Time.y`.
    camera: vec4<f32>,
    // The sun's colour (`_LightColor0`, the vertex lights' directional one),
    // and the share of it the shaders that take no shadow are lit by
    // (`1 - _DirectionLightShadow`).
    light: vec4<f32>,
    // The way to the sun in the view's axes.
    light_view: vec4<f32>,
    // The vertex-lit `glstate_lightmodel_ambient` (half the level's flat
    // ambient), in gamma.
    ambient: vec4<f32>,
    // The engine's own harmonics' constant (the flat ambient made linear).
    probe: vec4<f32>,
    // `_EFT_Ambient`, and `_LocalMinimalAmbientLight`.
    level: vec4<f32>,
    // The material's colour (`_TintColor` or `_Color`) and `_MainTex_ST`.
    tint: vec4<f32>,
    st: vec4<f32>,
    // The material's figures, each shader its own (`explosion_view`).
    a: vec4<f32>,
    b: vec4<f32>,
    // The time of day's harmonics, packed (the three `a`, the three `b`,
    // `c`), in the game's axes.
    sh: array<vec4<f32>, 7>,
    // More of the material's figures (the grenade smoke's
    // `_IndoorTintColor`, `_TODAttenuation`).
    c: vec4<f32>,
    d: vec4<f32>,
    // x: how the corners are lit by the vertex lights (0 not, 1 the grenade
    // smoke's, 2 the vertex-lit trails').
    lit: vec4<f32>,
    // The grenade smoke's level figures: `_MinAmbientColor`,
    // `_SunVisibility`; `unity_AmbientSky`, `_ClearSky`;
    // `_TopHorizontSkyColor`, `_ForceIndoor`; `_ForceIndoorIntensity`.
    smoke_a: vec4<f32>,
    smoke_b: vec4<f32>,
    smoke_c: vec4<f32>,
    smoke_d: vec4<f32>,
    // The engine's eight vertex lights in the app's axes (a place with w 1,
    // a directional one's way with w 0), their colours, fall-offs and cones.
    vl_position: array<vec4<f32>, 8>,
    vl_color: array<vec4<f32>, 8>,
    vl_atten: array<vec4<f32>, 8>,
    vl_spot: array<vec4<f32>, 8>,
}

@group(0) @binding(0) var<uniform> effect: Effect;
@group(0) @binding(1) var main_picture: texture_2d<f32>;
@group(0) @binding(2) var main_sampler: sampler;
@group(0) @binding(3) var noise_picture: texture_2d<f32>;
@group(0) @binding(4) var noise_sampler: sampler;
@group(0) @binding(5) var bump_picture: texture_2d<f32>;
@group(0) @binding(6) var bump_sampler: sampler;
@group(0) @binding(7) var scene_depth: texture_2d<f32>;
// The frame as it stood before this system was drawn (a shader's unnamed
// `GrabPass`: copied afresh for each), for the distortions; another picture
// for the rest.
@group(0) @binding(8) var grab_picture: texture_2d<f32>;
@group(0) @binding(9) var grab_sampler: sampler;
// A material's cube (`_Cube`), white where it has none.
@group(0) @binding(10) var cube_picture: texture_cube<f32>;
@group(0) @binding(11) var cube_sampler: sampler;

struct Corner {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) normal: vec3<f32>,
}

struct Out {
    @builtin(position) clip: vec4<f32>,
    // The picture's place, `_MainTex_ST` applied; and as it came.
    @location(0) uv: vec4<f32>,
    @location(1) color: vec4<f32>,
    @location(2) normal: vec3<f32>,
    // Its depth from the eye.
    @location(3) eye: f32,
    // Its place on the frame's copy as the grab passes' vertex programs
    // hand it on (`ComputeGrabScreenPos`, the copy upside down as the
    // engine's): across and down times the clip's w, the clip's w over the
    // corner's distance from the camera, the clip's w.
    @location(4) grab: vec4<f32>,
    // `DistortionFresnel`'s weight at the corner (from `a`: `_FresnelPow`,
    // `_FresnelR0`).
    @location(5) weight: f32,
    // The vertex lights' light at the corner (`lit.x` not nought).
    @location(6) lit: vec3<f32>,
    // The corner's place in the world.
    @location(7) world: vec3<f32>,
}

// The engine's vertex lights at a place (Unity's vertex-lit sum): the first
// `count`, each its colour times `1 / (1 + d² × atten.z)` times its cone's
// `saturate((max(l · spot, 0) − atten.x) × atten.y)`; a directional one's
// way taken as its place at a distance of one, or, where `times_w`, times
// its own w (nought: the grenade smoke's).
fn vertex_lights_at(at: vec3<f32>, count: i32, times_w: bool) -> vec3<f32> {
    var sum = vec3<f32>(0.0);
    for (var i = 0; i < count; i++) {
        let p = effect.vl_position[i];
        let to_light = p.xyz - at * p.w;
        let d2 = dot(to_light, to_light);
        let way = to_light * inverseSqrt(max(d2, 1e-12));
        var atten = 1.0 / (1.0 + d2 * effect.vl_atten[i].z);
        let rho = max(dot(way, effect.vl_spot[i].xyz), 0.0);
        atten *= clamp((rho - effect.vl_atten[i].x) * effect.vl_atten[i].y, 0.0, 1.0);
        if times_w {
            atten *= p.w;
        }
        sum += effect.vl_color[i].rgb * atten;
    }
    return sum;
}

// The camera's place in the world, from the view's matrix (a turn and a
// move).
fn camera_place() -> vec3<f32> {
    let v = effect.view_from_world;
    let turn = mat3x3<f32>(v[0].xyz, v[1].xyz, v[2].xyz);
    return -(transpose(turn) * v[3].xyz);
}

@vertex
fn vertex(c: Corner) -> Out {
    var out: Out;
    out.clip = effect.clip_from_world * vec4<f32>(c.position, 1.0);
    out.uv = vec4<f32>(c.uv * effect.st.xy + effect.st.zw, c.uv);
    out.color = c.color;
    out.normal = c.normal;
    out.world = c.position;
    out.eye = -(effect.view_from_world * vec4<f32>(c.position, 1.0)).z;
    let to_camera = camera_place() - c.position;
    let w = out.clip.w;
    out.grab = vec4<f32>((out.clip.x + w) * 0.5, (w - out.clip.y) * 0.5, w / max(length(to_camera), 1e-6), w);
    // The way to the camera and the normal, each made a unit (the game's in
    // the mesh's own axes; the same angle in the world's, the mesh only
    // turned and evenly sized).
    let f = pow(1.0 - abs(dot(normalize(c.normal), normalize(to_camera))), effect.a.y);
    out.weight = clamp((1.0 - effect.a.z) * f + effect.a.z, 0.0, 1.0);
    out.lit = vec3<f32>(0.0);
    if effect.lit.x == 1.0 {
        // `Particles/Smoke Distorted Grenade`: at the particle's centre (a
        // vertex stream its renderer does not give: `PROVISIONAL`, the
        // engine's nought, the world's origin), the ambient (the largest of
        // `_MinAmbientColor`, `unity_AmbientSky`, `_TopHorizontSkyColor` held
        // to one, and `_AddAmbient` toward its indoor share by
        // `_ForceIndoor`) times the time of day's attenuation, plus the first
        // six vertex lights' points and spots, held within 0 and 1, times the
        // colour's red.
        let add = clamp(mix(effect.b.x, effect.b.x * effect.smoke_d.x, effect.smoke_c.w), 0.0, 1.0);
        let ambient = max(max(max(effect.smoke_a.rgb, effect.smoke_b.rgb), clamp(effect.smoke_c.rgb, vec3<f32>(0.0), vec3<f32>(1.0))), vec3<f32>(add));
        let tod = mix(effect.d.y, effect.d.x, effect.smoke_a.w) * mix(effect.d.w, effect.d.z, effect.smoke_b.w);
        let total = clamp(ambient * tod + vertex_lights_at(vec3<f32>(0.0), 6, true), vec3<f32>(0.0), vec3<f32>(1.0));
        out.lit = total * c.color.r;
    } else if effect.lit.x == 2.0 {
        // `Custom/Particles-AdditiveVertLit`: twice the vertex ambient, at
        // least `_MinAmbient`, plus the eight vertex lights at the corner.
        out.lit = max(2.0 * effect.ambient.rgb, vec3<f32>(effect.a.y)) + vertex_lights_at(c.position, 8, false);
    }
    return out;
}

// The scene's depth from the eye at this pixel (`LinearEyeDepth` of the
// camera's depth texture), the depth reversed as the app's.
fn scene_eye(clip: vec4<f32>) -> f32 {
    let d = textureLoad(scene_depth, vec2<i32>(clip.xy), 0).r;
    return effect.camera.x / max(d, 1e-12);
}

// Every pass's `ZTest LEqual` (none writes the depth): a fragment behind the
// scene is not drawn.
fn depth_test(in: Out) {
    if in.eye > scene_eye(in.clip) {
        discard;
    }
}

// Soft against the scene: the eye's depths' difference times `inv_fade`.
fn soft(in: Out, inv_fade: f32) -> f32 {
    return clamp((scene_eye(in.clip) - in.eye) * inv_fade, 0.0, 1.0);
}

// Unity's linear to gamma (`LinearToGammaSpace`).
fn gamma(c: vec3<f32>) -> vec3<f32> {
    return max(1.055 * pow(max(c, vec3<f32>(0.0)), vec3<f32>(0.416667)) - vec3<f32>(0.055), vec3<f32>(0.0));
}

// The noise read three ways (`_Time.x` × speed × 0.96, 1.0, 1.03; up and
// along two slants), a pair of its channels each, summed less 1.5.
fn noise_move(uv: vec2<f32>, speed: f32) -> vec2<f32> {
    let t = effect.camera.y * speed * vec3<f32>(0.96, 1.0, 1.03);
    let a = textureSample(noise_picture, noise_sampler, uv + vec2<f32>(0.0, t.x)).xy;
    let b = textureSample(noise_picture, noise_sampler, uv + t.y * vec2<f32>(0.866, -0.5)).yz;
    let c = textureSample(noise_picture, noise_sampler, uv + t.z * vec2<f32>(-0.866, -0.5)).zw;
    return a + b + c - vec2<f32>(1.5);
}

// `Particles/Smoke Distorted` (a: `_AnimationSpeed`, `_Distortion`,
// `_InvFade`, `_Ambient`; `_TintColor`): lit by the ambient (twice, at
// least `_Ambient`) and the sun, times the colour's red; distorted by its
// green; the fog none.
@fragment
fn smoke_distorted(in: Out) -> @location(0) vec4<f32> {
    depth_test(in);
    let light = max(2.0 * effect.ambient.rgb, vec3<f32>(effect.a.w)) + effect.light.rgb * effect.light.w;
    let moved = noise_move(in.uv.zw, effect.a.x) * (in.color.g * effect.a.y);
    let t = textureSample(main_picture, main_sampler, in.uv.zw + moved);
    let lit = vec4<f32>(light * in.color.r, soft(in, effect.a.z) * in.color.a) * effect.tint;
    return t * lit;
}

// `Particles/Smoke Bumped` (a: `_AnimationSpeed`, `_Distortion`,
// `_NoiseScale`, `_InvFade`; b: `_NearCameraFade`, `_GIIndirect`,
// `_Scaterring`; `_Color`): the picture distorted by the noise (by the
// colour's blue), its normal from the bump picture taken in the view's
// axes; the sun's light by that normal, wrapped by `_Scaterring`; the
// ambient's (gamma) times `_GIIndirect`; faded softly and near the camera.
@fragment
fn smoke_bumped(in: Out) -> @location(0) vec4<f32> {
    depth_test(in);
    let noise_uv = in.uv.xy * effect.a.z;
    let moved = noise_move(noise_uv, effect.a.x) * (in.color.b * effect.a.y);
    let at = in.uv.xy + moved;
    let albedo = textureSample(main_picture, main_sampler, at) * effect.tint * vec4<f32>(in.color.r, in.color.r, in.color.r, in.color.a);
    let packed = textureSample(bump_picture, bump_sampler, at);
    let xy = vec2<f32>(packed.r * packed.a, packed.g) * 2.0 - vec2<f32>(1.0);
    let n = vec3<f32>(xy, sqrt(1.0 - min(dot(xy, xy), 1.0)));
    let facing = max(dot(n, effect.light_view.xyz), 0.0);
    let wrapped = facing + effect.b.z * (1.0 - facing);
    let sun = wrapped * albedo.rgb * effect.light.rgb * effect.light.w;
    let ambient = albedo.rgb * gamma(effect.probe.rgb) * effect.b.y;
    let alpha = albedo.a * soft(in, effect.a.w) * clamp(in.eye * effect.b.x, 0.0, 1.0);
    return vec4<f32>(sun + ambient, alpha);
}

// `Particles/SuperAdditive` (a: `_Energy`): the picture times the colour
// times its energy.
@fragment
fn super_additive(in: Out) -> @location(0) vec4<f32> {
    depth_test(in);
    return in.color * effect.a.x * textureSample(main_picture, main_sampler, in.uv.xy);
}

// `Legacy Shaders/Particles/Additive` (a: `_InvFade`; `_TintColor`): twice
// the colour times its tint times the picture, its alpha soft; the fog
// none.
@fragment
fn legacy_additive(in: Out) -> @location(0) vec4<f32> {
    depth_test(in);
    let c = vec4<f32>(in.color.rgb, in.color.a * soft(in, effect.a.x)) * effect.tint * 2.0;
    let o = c * textureSample(main_picture, main_sampler, in.uv.xy);
    return vec4<f32>(o.rgb, clamp(o.a, 0.0, 1.0));
}

// `CustomFX/HDRFireParticle` (a: `_HDRAmount`; `_TintColor`): the
// picture's alpha times the colour, its tint and `_HDRAmount`, by the soft
// fade (the depths' plain difference) times the colour's alpha, twice.
@fragment
fn hdr_fire(in: Out) -> @location(0) vec4<f32> {
    depth_test(in);
    let fade = clamp(scene_eye(in.clip) - in.eye, 0.0, 1.0) * in.color.a;
    let a = textureSample(main_picture, main_sampler, in.uv.xy).a;
    let rgb = in.color.rgb * a * effect.tint.rgb * (fade * effect.a.x) * fade;
    return vec4<f32>(rgb, fade * a * effect.tint.a);
}

// `Global Fog/Alpha Blended Lighted` (`LDR`, `GAMMA`; a: `_InvFade`;
// `_TintColor`): lit by the ambient (twice) and the whole sun, at least the
// level's least (`_LocalMinimalAmbientLight`) and at most 0.7, times its
// tint (the colour's alpha alone); the picture times that and `1 +
// _EFT_Ambient`, its alpha soft; the sky's fog none.
@fragment
fn fog_dust(in: Out) -> @location(0) vec4<f32> {
    depth_test(in);
    let light = min(max(2.0 * effect.ambient.rgb, vec3<f32>(effect.level.w)) + effect.light.rgb, vec3<f32>(0.7));
    let c = vec4<f32>(light, in.color.a) * effect.tint;
    let t = textureSample(main_picture, main_sampler, in.uv.xy);
    let rgb = c.rgb * t.rgb * (vec3<f32>(1.0) + effect.level.rgb);
    return vec4<f32>(rgb, c.a * soft(in, effect.a.x) * t.a);
}

// The time of day's harmonics at a way, in the game's axes (the shaders'
// sum over the packing: `trp_sim::ambient::Harmonics::at`).
fn harmonics_at(n: vec3<f32>) -> vec3<f32> {
    let one = vec4<f32>(n, 1.0);
    let two = vec4<f32>(n.x * n.y, n.y * n.z, n.z * n.z, n.z * n.x);
    let c = effect.sh[6].xyz * (n.x * n.x - n.y * n.y);
    return vec3<f32>(dot(effect.sh[0], one) + dot(effect.sh[3], two), dot(effect.sh[1], one) + dot(effect.sh[4], two), dot(effect.sh[2], one) + dot(effect.sh[5], two)) + c;
}

// `Legacy Shaders/Transparent/Cutout/Bumped Diffuse` (a: `_Cutoff`; b:
// `_BumpMap_ST`; `_Color`): drawn into the game's G-buffer (its deferred
// pass) and lit there. The picture times the colour, cut where its alpha
// less `_Cutoff` is below nought (the particle's colour not taken); its
// normal from the bump picture, taken in the view's axes as the other
// billboards'; lit by the sun by that normal and by the screen's ambient at
// it (the time of day's harmonics, held at nought and taken to gamma, as
// `Hidden/WriteScreenAmbient`); opaque. `PROVISIONAL`: no shadow, and the
// explosion's own lights not taken.
@fragment
fn cutout_bumped(in: Out) -> @location(0) vec4<f32> {
    depth_test(in);
    let t = textureSample(main_picture, main_sampler, in.uv.xy);
    let packed = textureSample(bump_picture, bump_sampler, in.uv.zw * effect.b.xy + effect.b.zw);
    if (t.a * effect.tint.a - effect.a.x < 0.0) {
        discard;
    }
    let albedo = t.rgb * effect.tint.rgb;
    let xy = vec2<f32>(packed.r * packed.a, packed.g) * 2.0 - vec2<f32>(1.0);
    let n = vec3<f32>(xy, sqrt(1.0 - min(dot(xy, xy), 1.0)));
    let facing = max(dot(n, effect.light_view.xyz), 0.0);
    let to_world = transpose(mat3x3<f32>(effect.view_from_world[0].xyz, effect.view_from_world[1].xyz, effect.view_from_world[2].xyz));
    let w = to_world * n;
    let ambient = gamma(harmonics_at(vec3<f32>(w.x, w.y, -w.z)));
    return vec4<f32>(albedo * (effect.light.rgb * facing + ambient), 1.0);
}

// `Particles/Smoke Distorted Grenade` (a: `_AnimationSpeed`, `_Distortion`,
// `_InvFade`, `_Indoor`; `_TintColor`, c: `_IndoorTintColor`): the noise
// read three ways as the fumes' (at the particle's own picture place), the
// picture read there moved by the noise times the colour's green times
// `_Distortion`; times the corner's light times the tint (taken toward the
// indoor one by `_Indoor`, back by `_ForceIndoor`), its alpha the tint's
// times the colour's times a half times the soft fade; the alpha times 4.
// The sky's scattering fog toward `_ScatteringTex` not built (its share at
// most 0.1% within 20 m at Customs' density: firing spec).
@fragment
fn smoke_grenade(in: Out) -> @location(0) vec4<f32> {
    depth_test(in);
    let n = noise_move(in.uv.zw, effect.a.x);
    let t = textureSample(main_picture, main_sampler, in.uv.zw + n * in.color.g * effect.a.y);
    let tint = mix(mix(effect.tint, effect.c, effect.a.w), effect.tint, effect.smoke_c.w);
    let c = t * vec4<f32>(in.lit * tint.rgb, tint.a * in.color.a * 0.5 * soft(in, effect.a.z));
    return vec4<f32>(c.rgb, clamp(c.a * 4.0, 0.0, 1.0));
}

// `Custom/Particles-AdditiveVertLit` (a: `_InvFade`, `_MinAmbient`;
// `_TintColor`): the picture times the tint, times the corner's light and
// the colour; its alpha times the soft fade and the colour's.
@fragment
fn additive_vert_lit(in: Out) -> @location(0) vec4<f32> {
    depth_test(in);
    let t = textureSample(main_picture, main_sampler, in.uv.xy) * effect.tint;
    return vec4<f32>(t.rgb * in.lit * in.color.rgb, soft(in, effect.a.x) * in.color.a * t.a);
}

// `Particles/AdditiveSimple` (its soft variant, which reads no depth):
// twice the colour times the picture.
@fragment
fn additive_simple(in: Out) -> @location(0) vec4<f32> {
    depth_test(in);
    return 2.0 * in.color * textureSample(main_picture, main_sampler, in.uv.xy);
}

// `Legacy Shaders/Transparent/Diffuse` (`_Color`), its forward base pass on a
// billboard whose normals lean straight to the camera (its renderer's
// `m_NormalDirection` 1, every one of the install's hits'): the picture
// times the colour, lit by the sun (`_LightColor0`, the probe's occlusion
// none) by the facing and by the time of day's harmonics at the normal,
// held at nought and taken to gamma (`ShadeSH9` in the vertex program); its
// alpha the picture's times the colour's. The particle's own colour is
// not taken (the shader has no vertex colour). `PROVISIONAL`: the added
// pass for other lights not built.
@fragment
fn transparent_diffuse(in: Out) -> @location(0) vec4<f32> {
    depth_test(in);
    let t = textureSample(main_picture, main_sampler, in.uv.xy) * effect.tint;
    let n = vec3<f32>(0.0, 0.0, 1.0);
    let facing = max(dot(n, effect.light_view.xyz), 0.0);
    let to_world = transpose(mat3x3<f32>(effect.view_from_world[0].xyz, effect.view_from_world[1].xyz, effect.view_from_world[2].xyz));
    let w = to_world * n;
    let ambient = gamma(harmonics_at(vec3<f32>(w.x, w.y, -w.z)));
    return vec4<f32>(t.rgb * (effect.light.rgb * facing + ambient), t.a);
}

// Unity's `Standard` on a particle (`_Color`; a: `_Metallic`, `_Glossiness`,
// fade (2) or opaque; b: `_EmissionColor` where `_EMISSION` is on), its
// forward base pass for `DIRECTIONAL LIGHTPROBE_SH` with `_ALPHABLEND_ON
// _EMISSION` (the fade) or none (the opaque, drawn here forward where the
// game draws it into its G-buffer), as the game's compiled program has it
// (`BRDF1`): the picture times `_Color` (the particle's own colour not
// taken: the program has none); its specular colour from the gamma
// dielectric 0.220916 toward the albedo by `_Metallic`, its diffuse the
// albedo times 0.779084 less `_Metallic` of it; on a billboard whose normal
// is straight to the camera, the sun (`_LightColor0`) by the Disney diffuse
// and by the GGX specular (Smith's joint visibility, the distribution,
// times pi, its square root in gamma, by the facing), its Fresnel by the
// light and the half way; the time of day's harmonics at the normal, held
// at nought and taken to gamma, on the diffuse; plus the emission (its map
// none: white, times `_EmissionColor`). `PROVISIONAL`: the reflection
// probe's part nought (the recorded morning's default reflection has an
// intensity of nought; the level's probes not read); the opaque one's
// shadows, and both's other lights, not taken.
fn standard_lit(in: Out) -> vec4<f32> {
    let t = textureSample(main_picture, main_sampler, in.uv.xy);
    let albedo = t.rgb * effect.tint.rgb;
    let metallic = effect.a.x;
    let smoothness = effect.a.y;
    let spec_color = mix(vec3<f32>(0.220916), albedo, metallic);
    let one_minus_reflectivity = 0.779084 - metallic * 0.779084;
    let diffuse_color = albedo * one_minus_reflectivity;
    let v = effect.view_from_world;
    let to_world = transpose(mat3x3<f32>(v[0].xyz, v[1].xyz, v[2].xyz));
    let n = normalize(to_world * vec3<f32>(0.0, 0.0, 1.0));
    let eye = normalize(in.world - camera_place());
    let l = normalize(to_world * effect.light_view.xyz);
    let pr = 1.0 - smoothness;
    let w = n;
    let ambient = gamma(harmonics_at(vec3<f32>(w.x, w.y, -w.z)));
    let h = (l - eye) * inverseSqrt(max(dot(l - eye, l - eye), 0.001));
    let nv = dot(n, -eye);
    let nl = clamp(dot(n, l), 0.0, 1.0);
    let nh = clamp(dot(n, h), 0.0, 1.0);
    let lh = clamp(dot(l, h), 0.0, 1.0);
    let fd90_less_one = 2.0 * lh * lh * pr - 0.5;
    let light_scatter = 1.0 + fd90_less_one * pow(1.0 - nl, 5.0);
    let view_scatter = 1.0 + fd90_less_one * pow(1.0 - abs(nv), 5.0);
    let diffuse_term = light_scatter * view_scatter * nl;
    let r = max(pr * pr, 0.002);
    let lambda_v = nl * (abs(nv) * (1.0 - r) + r);
    let lambda_l = abs(nv) * (nl * (1.0 - r) + r);
    let visibility = 0.5 / (lambda_v + lambda_l + 0.00001);
    let a2 = r * r;
    let d = (nh * a2 - nh) * nh + 1.0;
    let distribution = a2 * 0.31831 / (d * d);
    var specular = sqrt(max(visibility * distribution * 3.141593, 0.0001)) * nl;
    if dot(spec_color, spec_color) == 0.0 {
        specular = 0.0;
    }
    let fresnel = spec_color + (vec3<f32>(1.0) - spec_color) * pow(1.0 - lh, 5.0);
    let light = effect.light.rgb;
    let rgb = diffuse_color * (ambient + light * diffuse_term) + specular * light * fresnel + effect.b.rgb;
    return vec4<f32>(rgb, t.a * effect.tint.a);
}

@fragment
fn standard_fade(in: Out) -> @location(0) vec4<f32> {
    depth_test(in);
    return standard_lit(in);
}

@fragment
fn standard_opaque(in: Out) -> @location(0) vec4<f32> {
    depth_test(in);
    return vec4<f32>(standard_lit(in).rgb, 1.0);
}

// `EFT/BumpedGlass` (`_ReflectColor`; a: `_FresPow`, `_FresAdd`, `_Specular`,
// `_Gloss`; b: `_BumpMap_ST`; c: `_Shininess`, `_GlobalReflectionStrength`;
// d: `_SpecColor`), its forward base pass for `DIRECTIONAL LIGHTPROBE_SH`,
// blended `One One`, no diffuse: on a billboard facing the camera (its
// tangent frame the view's: `PROVISIONAL`, the tangent's sign), the bump
// picture's normal (the green, and the alpha times the red); a Fresnel
// `pow(1 − view·normal, _FresPow) + _FresAdd` times the picture's alpha and
// `_Specular`; the material's cube read along the way from the camera
// reflected about the normal (in the game's axes), times that Fresnel and
// `_ReflectColor` (the reflection probe's part, `_GlobalReflectionStrength`
// times it, nought here); plus the sun's colour times `_SpecColor` times the
// normal and the half way's product to the power `_Shininess × 128`, times
// the Fresnel and `_Gloss`; its alpha 1.
@fragment
fn bumped_glass(in: Out) -> @location(0) vec4<f32> {
    depth_test(in);
    let t = textureSample(main_picture, main_sampler, in.uv.xy);
    let packed = textureSample(bump_picture, bump_sampler, in.uv.zw * effect.b.xy + effect.b.zw);
    let xy = vec2<f32>(packed.r * packed.a, packed.g) * 2.0 - vec2<f32>(1.0);
    let n = vec3<f32>(xy, sqrt(1.0 - min(dot(xy, xy), 1.0)));
    let v = effect.view_from_world;
    let to_world = transpose(mat3x3<f32>(v[0].xyz, v[1].xyz, v[2].xyz));
    let place = (v * vec4<f32>(in.world, 1.0)).xyz;
    let to_eye = normalize(-place);
    let fresnel = (pow(max(1.0 - dot(to_eye, normalize(n)), 0.0), effect.a.x) + effect.a.y) * t.a * effect.a.z;
    let gloss = fresnel * effect.a.w;
    let n_world = normalize(to_world * n);
    let eye_world = to_world * to_eye;
    let r = reflect(-eye_world, n_world);
    let cube = textureSample(cube_picture, cube_sampler, vec3<f32>(r.x, r.y, -r.z)).rgb;
    let reflected = cube * fresnel * effect.tint.rgb;
    let l = normalize(to_world * effect.light_view.xyz);
    let half_way = normalize(eye_world + l);
    let nh = max(dot(n_world, half_way), 0.0);
    let highlight = pow(nh, effect.c.x * 128.0) * gloss;
    return vec4<f32>(effect.light.rgb * effect.d.rgb * highlight + reflected, 1.0);
}

// `Legacy Shaders/Particles/Multiply` (the body's blood splash), its soft
// variant: `1 + a (colour × picture − 1)`, `a` the picture's alpha times the
// colour's softened, blended as the frame times it (`Blend Zero SrcColor`).
@fragment
fn multiply(in: Out) -> @location(0) vec4<f32> {
    depth_test(in);
    let c = vec4<f32>(in.color.rgb, in.color.a * soft(in, effect.a.x));
    let t = textureSample(main_picture, main_sampler, in.uv.xy);
    return (c * t - vec4<f32>(1.0)) * (c.a * t.a) + vec4<f32>(1.0);
}

// `Legacy Shaders/Particles/Multiply (Double)` (the body's blood drops):
// `0.5 + a (2 × colour × picture − 0.5)` on the colour, its alpha `0.5 +
// a (a − 0.5)`, blended as twice the frame times it (`Blend DstColor
// SrcColor`).
@fragment
fn multiply_double(in: Out) -> @location(0) vec4<f32> {
    depth_test(in);
    let t = textureSample(main_picture, main_sampler, in.uv.xy);
    let a = in.color.a * soft(in, effect.a.x) * t.a;
    let doubled = vec4<f32>(t.rgb * in.color.rgb * 2.0, a);
    return (doubled - vec4<f32>(0.5)) * a + vec4<f32>(0.5);
}

// `Legacy Shaders/Particles/Additive (Soft)` (a: `_InvFade`; no tint): the
// picture times the colour, its alpha soft against the scene, its colour
// by its own alpha (`One OneMinusSrcColor`).
@fragment
fn additive_soft(in: Out) -> @location(0) vec4<f32> {
    depth_test(in);
    let t = textureSample(main_picture, main_sampler, in.uv.xy) * vec4<f32>(in.color.rgb, in.color.a * soft(in, effect.a.x));
    return vec4<f32>(t.rgb * t.a, t.a);
}

// `Effects/Explosions/DistortionFresnel` (a: `_BumpAmt`, `_FresnelPow`,
// `_FresnelR0`), its second pass: the frame's copy read where the pixel is,
// moved by the corner's weight times `_BumpAmt` pixels times the particle's
// alpha, to the right and down alike; times the particle's colour, its
// alpha held within 0 and 1. Its material's queue (1000) has it drawn in
// the game after the opaque scene and before the sky, which is drawn only
// where nothing was: where nothing is, the sky stays as drawn (`PROVISIONAL`:
// a read that falls on the sky takes the app's sky, where the game's copy
// holds what the camera cleared to).
@fragment
fn distortion_wave(in: Out) -> @location(0) vec4<f32> {
    depth_test(in);
    if textureLoad(scene_depth, vec2<i32>(in.clip.xy), 0).r <= 0.0 {
        discard;
    }
    let texel = 1.0 / vec2<f32>(textureDimensions(grab_picture));
    let shift = clamp(in.weight, 0.0, 1.0) * texel * effect.a.x * in.color.a;
    let at = (shift * in.grab.w + in.grab.xy) / in.grab.w;
    let c = textureSampleLevel(grab_picture, grab_sampler, at, 0.0) * in.color;
    return vec4<f32>(c.rgb, clamp(c.a, 0.0, 1.0));
}

// `Effects/Explosions/Distortion/ParticlesCutOut` (a: `_BumpAmt`,
// `_InvFade`), its second pass: the picture's two parts (its alpha times
// its red, its green) from −1 to 1; cut where their sum's size less 0.01,
// times 5 (`w`), is under a half; the frame's copy read where the pixel is,
// moved by the parts times `_BumpAmt` pixels times `w`, the particle's
// alpha and the soft fade, over the corner's distance from the camera; drawn
// with `w` times the copy's alpha.
@fragment
fn heat_distortion(in: Out) -> @location(0) vec4<f32> {
    depth_test(in);
    var fade = in.color.a;
    if effect.a.y > 0.0001 {
        fade = soft(in, effect.a.y) * in.color.a;
    }
    let t = textureSample(main_picture, main_sampler, in.uv.xy);
    let parts = vec2<f32>(t.a * t.r, t.g) * 2.0 - vec2<f32>(1.0);
    let w = clamp((abs(parts.x + parts.y) - 0.01) * 5.0, 0.0, 1.0);
    if w < 0.5 {
        discard;
    }
    let texel = 1.0 / vec2<f32>(textureDimensions(grab_picture));
    let shift = parts * effect.a.x * texel * fade * w;
    let at = (shift * in.grab.z + in.grab.xy) / in.grab.w;
    let c = textureSampleLevel(grab_picture, grab_sampler, at, 0.0);
    return vec4<f32>(c.rgb, clamp(w * c.a, 0.0, 1.0));
}

// `Particles/HSL Additive` (a: `_InvFade`; b: `_HueRGB`; `_TintColor`):
// twice the colour times its tint times the picture, plus the hue; its
// alpha soft.
@fragment
fn hsl_additive(in: Out) -> @location(0) vec4<f32> {
    depth_test(in);
    let c = vec4<f32>(in.color.rgb, in.color.a * soft(in, effect.a.x)) * 2.0 * effect.tint;
    let t = textureSample(main_picture, main_sampler, in.uv.xy);
    return vec4<f32>(c.rgb * t.rgb + effect.b.xyz + vec3<f32>(effect.b.w), c.a * t.a);
}
