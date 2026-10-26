// A time-of-day sky (`tod_sky_view.rs`; lighting spec, "Customs by its
// time of day"): `Time of Day/Atmosphere` on the level's own dome mesh, the
// light scattered toward the eye worked out at its vertices and the phases
// at each pixel, added over the sun's disc (`Time of Day/Sun`, its quad's
// pattern at each pixel) and the moon (`Time of Day/Moon`, its sphere's
// picture lit by the sun, opaque over the disc as the game draws it after)
// on a black ground; then made linear by the 2.2 power the frame is turned
// back into the game's numbers by (`lighting`).

#import bevy_pbr::mesh_view_bindings::view

struct TodSky {
    // `TOD_SunSkyColor`, `TOD_MoonSkyColor`, `TOD_GroundColor`,
    // `TOD_MoonHaloColor` (rgb).
    sun_sky: vec4<f32>,
    moon_sky: vec4<f32>,
    ground: vec4<f32>,
    moon_halo: vec4<f32>,
    // `TOD_LocalSunDirection`, `TOD_LocalMoonDirection`, in the game's axes.
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
    // The sun's mesh: its colour (rgb) and its quad's half width where the
    // sun is at one (a); `TOD_SunMeshContrast`, `TOD_SunMeshBrightness`,
    // and whether it is drawn.
    sun_mesh: vec4<f32>,
    sun_mesh_figures: vec4<f32>,
    // The moon's object's axes in the dome's (right, up, forward), its
    // sphere's radius where it is at one (right's w), whether it is drawn
    // (up's w), `TOD_MoonMeshContrast` (forward's w); its brightness; its
    // picture's tiling (`_MainTex_ST`); `TOD_SunDirection`.
    moon_right: vec4<f32>,
    moon_up: vec4<f32>,
    moon_forward: vec4<f32>,
    moon_figures: vec4<f32>,
    moon_tiling: vec4<f32>,
    sun_world: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> sky: TodSky;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var moon_picture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var moon_sampler: sampler;

struct Vertex {
    // The dome's vertex, in the game's axes.
    @location(0) position: vec3<f32>,
}

struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) rayleigh: vec3<f32>,
    @location(1) mie: vec3<f32>,
    @location(2) way: vec3<f32>,
}

// `ShaderScale`'s polynomial, without its quarter.
fn scale_exp(cos_angle: f32) -> f32 {
    let n = 1.0 - cos_angle;
    return exp(-0.00287 + n * (0.459 + n * (3.83 + n * (-6.8 + n * 5.25))));
}

@vertex
fn vertex(in: Vertex) -> Out {
    var out: Out;
    let v = in.position;
    // The vertex turned by the view alone and put at the far plane, as the
    // game's is (its own place taken for a way from the eye, whatever the
    // dome's place): the app's axes have z turned round, and its far plane
    // is at depth nought.
    out.clip = view.clip_from_world * vec4<f32>(v.x, v.y, -v.z, 0.0);
    out.clip.z = 0.0;
    out.way = v;

    // The height bent near the horizon, then four samples of the light
    // scattered along the way through the atmosphere's shell.
    var yb = select((v.y + 0.03125) * (v.y + 0.03125) * 8.0, v.y, v.y > 0.03125);
    yb = select(0.0, yb, v.y > -0.03125);
    let dir = vec3<f32>(v.x, yb, v.z);
    let x = sky.k_radius.x;
    let start_height = x + sky.k_scale.w;
    let far = sqrt(yb * yb * sky.k_radius.y + sky.k_radius.w - sky.k_radius.y) - x * yb;
    let start_offset = exp(-sky.k_scale.w * sky.k_scale.z) * scale_exp(yb);
    let sample_length = far * 0.25;
    let scaled_length = sample_length * sky.k_scale.x;
    var at = vec3<f32>(0.0, start_height, 0.0) + dir * sample_length * 0.5;
    let extinction = sky.k_4pi.w + sky.k_4pi.xyz;
    var gathered = vec3<f32>(0.0);
    for (var i = 0; i < 4; i++) {
        let height = length(at);
        let inv = 1.0 / height;
        let depth = exp((x - height) * sky.k_scale.z);
        let weight = scaled_length * depth;
        let camera_scale = scale_exp(dot(dir, at) * inv) * 0.25;
        let light_scale = scale_exp(dot(sky.sun.xyz, at) * inv);
        let scatter = start_offset * 0.25 + depth * (light_scale * 0.25 - camera_scale);
        gathered += exp(-scatter * extinction) * weight;
        at += dir * sample_length;
    }
    let lit = gathered * sky.sun_sky.rgb;
    out.rayleigh = lit * sky.k_sun.xyz;
    out.mie = lit * sky.k_sun.w;
    return out;
}

@fragment
fn fragment(in: Out) -> @location(0) vec4<f32> {
    let d = normalize(in.way);
    // The Rayleigh's and Mie's phases, the night sky, the moon's halo, the
    // fog's grey, the ground below the horizon; times the scattering
    // brightness, to the contrast's power, square-rooted, at most 4.
    let c_sun = dot(sky.sun.xyz, d);
    let mie_phase = sky.k_beta_mie.x * (1.0 + c_sun * c_sun) / pow(sky.k_beta_mie.z * c_sun + sky.k_beta_mie.y, 1.5);
    var c = mie_phase * in.mie + (0.75 + 0.75 * c_sun * c_sun) * in.rayleigh;
    c += sky.moon_sky.rgb - d.y * sky.moon_sky.rgb;
    c += sky.moon_halo.rgb * pow(max(dot(d, sky.moon.xyz), 0.0), sky.figures.w);
    let grey = (c.r + c.g + c.b) * 0.333;
    c += sky.figures.z * (vec3<f32>(grey) - c);
    let below = clamp(d.y * -0.8, 0.0, 1.0);
    c += below * below * (3.0 - 2.0 * below) * (sky.ground.rgb - c);
    let atmosphere = min(sqrt(pow(c * sky.figures.y, vec3<f32>(sky.figures.x))), vec3<f32>(4.0));

    // The sun's disc under it: where the way meets the sun's quad (square
    // to the sun at one), the quad's pattern by its distance from the
    // middle in halves of its width.
    var disc = vec3<f32>(0.0);
    let toward = dot(d, sky.sun.xyz);
    if (sky.sun_mesh_figures.z > 0.0 && toward > 0.0) {
        let on_plane = d / toward - sky.sun.xyz;
        let r = length(on_plane) / sky.sun_mesh.a;
        let t = clamp((r - 0.5) * 2.0, 0.0, 1.0);
        let falloff = 1.0 - t * t * (3.0 - 2.0 * t);
        let height = clamp((sky.sun.y + on_plane.y) * 25.0, 0.0, 1.0);
        disc = pow(falloff, sky.sun_mesh_figures.x) * height * sky.sun_mesh_figures.y * sky.sun_mesh.rgb;
    }
    // The moon over it, where the way meets its sphere (radius one,
    // scaled, about its place at one; the near side): its picture by the
    // sphere's own coordinates (across by the turn about its up from its
    // forward, up by the height), lit by the sun to the contrast's power,
    // faded near the horizon (twice its place's height, squared, at most
    // one), times its brightness. `PROVISIONAL`: a smooth sphere, where
    // the game's has sixteen rings of 32 sides.
    let m = sky.moon.xyz;
    let radius = sky.moon_right.w;
    let b = dot(d, m);
    let seen = b * b - (1.0 - radius * radius);
    // (Worked out at every pixel, for the picture's level of detail as the
    // game's sampling takes it; kept only on the sphere.)
    let n = normalize(d * (b - sqrt(max(seen, 0.0))) - m);
    let local = vec3<f32>(dot(n, sky.moon_right.xyz), dot(n, sky.moon_up.xyz), dot(n, sky.moon_forward.xyz));
    let u = 0.5 - atan2(local.x, local.z) / 6.283185;
    let v = asin(clamp(local.y, -1.0, 1.0)) / 3.141593 + 0.5;
    let picture = textureSample(moon_picture, moon_sampler, vec2<f32>(u, v) * sky.moon_tiling.xy + sky.moon_tiling.zw).rgb;
    var under = disc;
    if (sky.moon_up.w > 0.0 && seen >= 0.0 && b > 0.0) {
        let lit = pow(max(dot(n, sky.sun_world.xyz), 0.0), sky.moon_forward.w);
        let h = 2.0 * (m.y + radius * n.y);
        under = picture * lit * clamp(h * h, 0.0, 1.0) * sky.moon_figures.x;
    }
    // The game's numbers, made linear for the scene.
    return vec4<f32>(pow(max(under + atmosphere, vec3<f32>(0.0)), vec3<f32>(2.2)), 1.0);
}
