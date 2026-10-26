// A tactical device's laser (`tactical_view.rs`, drawn in `muzzle_view`'s
// pass; tactical-devices spec, "The look"), as the game's compiled shaders
// draw it, added to the frame in its gamma-encoded numbers (`gamma_ui`):
//
// - the beam (`Custom/LaserBeam`): a ribbon from the device as long as the
//   ray went, turned to face the camera, `BeamSize` to each side, four times
//   as bright at the emitter as at its end, its brightness by a 3D noise
//   drifting with the clock, times its soft spot across, its colour and its
//   factor (`_Factor`; `_IkFactor` under night vision, none here);
// - the dot (`Custom/LaserPoint`): a square on the surface hit, its size by
//   the camera's field of view and its distance, its picture times a fine
//   noise, its colour, its brightness and its factor.
//
// Neither is drawn behind what the scene drew there (`ZTest LEqual`, the
// depth reversed); the dot, lying on the surface, by a hair's tolerance
// (`PROVISIONAL`: the engine's depth test of a coplanar quad not read).

struct Laser {
    view_from_world: mat4x4<f32>,
    clip_from_view: mat4x4<f32>,
    // The camera's place, and its vertical field of view in degrees.
    camera: vec4<f32>,
    // The beam's start, and its half width (`BeamSize`).
    origin: vec4<f32>,
    // Its way, and its length (`_Distance`).
    axis: vec4<f32>,
    beam_color: vec4<f32>,
    // `_Intensity`, the factor, `_NoiseScale0`, the clock (`_Time.x`).
    beam: vec4<f32>,
    // `_SinTime.x`; the beam's place in the game's axes is the world's with
    // its z turned (y the game's up), for the noise.
    clock: vec4<f32>,
    // The dot's middle, and its size (`_Size`).
    dot_center: vec4<f32>,
    // The dot's square's two sides on the surface.
    dot_x: vec4<f32>,
    dot_y: vec4<f32>,
    dot_color: vec4<f32>,
    // `_Intensity`, the factor, `_MinFov`, `_MaxDist`.
    dot: vec4<f32>,
    // `_SizeFactorViewMin`, `_SizeFactorViewMax`.
    dot_view: vec4<f32>,
}

@group(0) @binding(0) var<uniform> laser: Laser;
@group(0) @binding(1) var main_picture: texture_2d<f32>;
@group(0) @binding(2) var main_sampler: sampler;
@group(0) @binding(3) var noise: texture_3d<f32>;
@group(0) @binding(4) var noise_sampler: sampler;
@group(0) @binding(5) var dot_picture: texture_2d<f32>;
@group(0) @binding(6) var dot_noise: texture_2d<f32>;
@group(0) @binding(7) var scene_depth: texture_2d<f32>;

struct Corner {
    // Its side (−1 or 1) and its share along (0 at the emitter, 1 at the
    // end); for the dot, its corner (−1 or 1 each way).
    @location(0) corner: vec2<f32>,
}

struct BeamOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) across: f32,
    @location(1) shade: f32,
    @location(2) c1: vec3<f32>,
    @location(3) c2: vec3<f32>,
}

@vertex
fn beam_vertex(c: Corner) -> BeamOut {
    let along = laser.axis.xyz;
    let p = laser.origin.xyz + along * (c.corner.y * laser.axis.w);
    let side = normalize(cross(along, laser.camera.xyz - p));
    let world = p + side * (c.corner.x * laser.origin.w);
    var out: BeamOut;
    out.clip = laser.clip_from_view * (laser.view_from_world * vec4<f32>(world, 1.0));
    out.across = c.corner.x * 0.5 + 0.5;
    out.shade = mix(4.0, laser.beam.x, c.corner.y);
    // The noise's places, in the game's axes.
    let g = vec3<f32>(p.x, p.y, -p.z) * laser.beam.z;
    let t = laser.beam.w;
    out.c1 = g + vec3<f32>(0.0, -0.5 * t, 0.0);
    out.c2 = 3.94 * g + vec3<f32>(2.0 * laser.clock.x, 0.0, t);
    return out;
}

fn hidden(clip: vec4<f32>, tolerance: f32) -> bool {
    let scene = textureLoad(scene_depth, vec2<i32>(clip.xy), 0).r;
    return clip.z < scene - tolerance;
}

@fragment
fn beam_fragment(in: BeamOut) -> @location(0) vec4<f32> {
    if (hidden(in.clip, 0.0)) {
        discard;
    }
    let a = textureSample(noise, noise_sampler, in.c1).r + 0.4 * textureSample(noise, noise_sampler, in.c2).r;
    let m = a * a * a;
    let spot = textureSample(main_picture, main_sampler, vec2<f32>(in.across, 0.5)).r;
    let rgb = m * laser.beam.y * spot * in.shade * laser.beam_color.rgb;
    return vec4<f32>(rgb, 0.0);
}

struct DotOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn dot_vertex(c: Corner) -> DotOut {
    let center = laser.dot_center.xyz;
    let fov_share = clamp((laser.camera.w - laser.dot.z) / (75.0 - laser.dot.z), 0.0, 1.0);
    let dc = length(laser.camera.xyz - center);
    let by_view = mix(laser.dot_view.x, laser.dot_view.y, min(dc, laser.dot.w) / laser.dot.w);
    let size = laser.dot_center.w * fov_share * by_view;
    let world = center + (laser.dot_x.xyz * c.corner.x + laser.dot_y.xyz * c.corner.y) * size;
    var out: DotOut;
    out.clip = laser.clip_from_view * (laser.view_from_world * vec4<f32>(world, 1.0));
    out.uv = c.corner * 0.5 + 0.5;
    return out;
}

@fragment
fn dot_fragment(in: DotOut) -> @location(0) vec4<f32> {
    if (hidden(in.clip, 1e-6)) {
        discard;
    }
    let picture = textureSample(dot_picture, main_sampler, in.uv).r;
    let grain = textureSample(dot_noise, noise_sampler, in.uv * 1234.5677).r;
    let rgb = picture * laser.dot_color.rgb * grain * laser.dot.x * laser.dot.y;
    return vec4<f32>(rgb, 0.0);
}
