// PROVISIONAL: the trail's side is mirrored by the axes' turn (its picture
// across it).
// A gun smoke's trail (`muzzle_view.rs`; firing spec, "The gun smoke"), as
// the game's compiled `Custom/MuzzleSmoke` draws it: each corner moved in
// the view across its point's direction on the screen by the trail's width
// and its diffusion, its picture along the trail by its number, faded in and
// out by its diffusion and softly against the scene's depth, blended over
// the frame by its alpha. On the frame in its gamma-encoded numbers, as the
// game blends it (`gamma_ui`).

struct Smoke {
    view_from_world: mat4x4<f32>,
    clip_from_view: mat4x4<f32>,
    // `_Width`, `_UVYScale`, `_DiffusionStrength`, `_StartFade`.
    a: vec4<f32>,
    // `_EndFade`, `_End`, `_InvFade`, `_Alpha`.
    b: vec4<f32>,
    // The camera's near plane.
    near: vec4<f32>,
}

@group(0) @binding(0) var<uniform> smoke: Smoke;
@group(0) @binding(1) var picture: texture_2d<f32>;
@group(0) @binding(2) var picture_sampler: sampler;
@group(0) @binding(3) var scene_depth: texture_2d<f32>;

struct Corner {
    @location(0) position: vec3<f32>,
    // Its side (−1 or 1), its number along the trail, its diffusion.
    @location(1) trail: vec3<f32>,
    @location(2) direction: vec3<f32>,
    @location(3) alpha: f32,
}

struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) fade: f32,
    // Its depth from the eye.
    @location(2) eye: f32,
}

@vertex
fn vertex(c: Corner) -> Out {
    let side = c.trail.x;
    let diffusion = c.trail.z;
    let across = normalize((smoke.view_from_world * vec4<f32>(c.direction, 0.0)).xy);
    let half_width = diffusion * diffusion * smoke.a.z + smoke.a.x;
    var view = smoke.view_from_world * vec4<f32>(c.position, 1.0);
    view = vec4<f32>(view.xy + vec2<f32>(-across.y, across.x) * side * half_width, view.z, view.w);
    var out: Out;
    out.clip = smoke.clip_from_view * view;
    out.fade = clamp(diffusion / smoke.a.w, 0.0, 1.0) * c.alpha * clamp((smoke.b.y - diffusion) / smoke.b.x, 0.0, 1.0);
    out.uv = vec2<f32>(max(side, 0.0), c.trail.y * smoke.a.y);
    out.eye = -view.z;
    return out;
}

@fragment
fn fragment(in: Out) -> @location(0) vec4<f32> {
    // Behind what the scene drew there: none (`ZTest LEqual`, the depth
    // reversed); near it, faded (`_InvFade`, by the eye's depths).
    let scene = textureLoad(scene_depth, vec2<i32>(in.clip.xy), 0).r;
    if (in.clip.z < scene) {
        discard;
    }
    let scene_eye = smoke.near.x / max(scene, 1e-12);
    let soft = clamp((scene_eye - in.eye) * smoke.b.z, 0.0, 1.0);
    let t = textureSample(picture, picture_sampler, in.uv);
    return vec4<f32>(t.rgb, soft * in.fade * smoke.b.w * t.a);
}
