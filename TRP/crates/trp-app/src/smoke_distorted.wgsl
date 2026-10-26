// The muzzles' fumes (`particles_view`; firing spec, "The muzzle's fumes"),
// as the game's compiled `Particles/Smoke Distorted` draws a particle (its
// first pass, with soft particles and `MBOIT_PARTICLES_OFF`): a billboard in
// the view's plane, its main picture read where a noise picture scrolled
// three ways moves it, lit by the scene's lights times its colour's red,
// distorted by its green, faded by its alpha and softly against the
// scene's depth, blended over the frame by its alpha. On the frame in its
// gamma-encoded numbers, as the game blends it (`gamma_ui`).

struct Fume {
    view_from_world: mat4x4<f32>,
    clip_from_view: mat4x4<f32>,
    // `_Time.x` (a twentieth of the seconds), `_AnimationSpeed`,
    // `_Distortion`, `_InvFade`.
    figures: vec4<f32>,
    // The light it is lit by (the ambient and the lights, `particles_view`).
    light: vec4<f32>,
    // `_TintColor`.
    tint: vec4<f32>,
    // The camera's near plane.
    near: vec4<f32>,
}

@group(0) @binding(0) var<uniform> fume: Fume;
@group(0) @binding(1) var picture: texture_2d<f32>;
@group(0) @binding(2) var picture_sampler: sampler;
@group(0) @binding(3) var noise: texture_2d<f32>;
@group(0) @binding(4) var noise_sampler: sampler;
@group(0) @binding(5) var scene_depth: texture_2d<f32>;

struct Corner {
    // The particle's place, its corner (each of ±0.5), its size, its turn
    // (radians) and its colour.
    @location(0) position: vec3<f32>,
    @location(1) corner: vec2<f32>,
    @location(2) size: f32,
    @location(3) turn: f32,
    @location(4) color: vec4<f32>,
}

struct Out {
    @builtin(position) clip: vec4<f32>,
    // The light times red, and alpha.
    @location(0) lit: vec4<f32>,
    // The picture's place, and the noise's first.
    @location(1) uv: vec4<f32>,
    // The noise's other two.
    @location(2) scrolled: vec4<f32>,
    // Green (the distortion's share), and its depth from the eye.
    @location(3) green_eye: vec2<f32>,
}

@vertex
fn vertex(c: Corner) -> Out {
    let centre = fume.view_from_world * vec4<f32>(c.position, 1.0);
    let s = sin(c.turn);
    let k = cos(c.turn);
    let along = c.corner * c.size;
    let offset = vec2<f32>(along.x * k - along.y * s, along.x * s + along.y * k);
    let view = vec4<f32>(centre.xy + offset, centre.z, 1.0);
    var out: Out;
    out.clip = fume.clip_from_view * view;
    // A billboard's picture runs from 0 to 1 across and up.
    let uv = c.corner + vec2<f32>(0.5, 0.5);
    let t = fume.figures.x * fume.figures.y * vec3<f32>(0.96, 1.0, 1.03);
    out.uv = vec4<f32>(uv, uv.x, uv.y + t.x);
    out.scrolled = vec4<f32>(uv + t.y * vec2<f32>(0.866, -0.5), uv + t.z * vec2<f32>(-0.866, -0.5));
    out.lit = vec4<f32>(fume.light.rgb * c.color.r, c.color.a);
    out.green_eye = vec2<f32>(c.color.g, -view.z);
    return out;
}

@fragment
fn fragment(in: Out) -> @location(0) vec4<f32> {
    // The noise read three ways, a different pair of its channels each.
    let a = textureSample(noise, noise_sampler, in.uv.zw).xy;
    let b = textureSample(noise, noise_sampler, in.scrolled.xy).yz;
    let c = textureSample(noise, noise_sampler, in.scrolled.zw).zw;
    let moved = (a + b + c - vec2<f32>(1.5, 1.5)) * (in.green_eye.x * fume.figures.z);
    let t = textureSample(picture, picture_sampler, in.uv.xy + moved);
    // Soft against the scene: the eye's depths (`_InvFade`).
    let scene = textureLoad(scene_depth, vec2<i32>(in.clip.xy), 0).r;
    let scene_eye = fume.near.x / max(scene, 1e-12);
    let soft = clamp((scene_eye - in.green_eye.y) * fume.figures.w, 0.0, 1.0);
    let lit = vec4<f32>(in.lit.rgb, soft * in.lit.a) * fume.tint;
    return t * lit;
}
