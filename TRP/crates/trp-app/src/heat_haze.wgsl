// The muzzles' heat haze (`distort_view`; firing spec, "The muzzle's heat
// haze"). Its particles, as the game's `Particles/DistortAndBlur` draws
// them (soft particles on) into the camera's distortion values: a
// billboard in the view's plane, its red and green a half plus the noise
// picture read two ways, scrolled (`_TextureMovement`), summed less one,
// times `_Distort` over its distance and its colour's red and green; its
// blue `_Blur` times its colour's blue; its alpha the alpha picture times
// its colour's; all times the soft fade (`_InvFade` × 10). The camera's
// passes that apply the values are `distort_camera.wgsl`.

struct Haze {
    view_from_world: mat4x4<f32>,
    clip_from_view: mat4x4<f32>,
    // `_Time.x`, `_Distort`, `_Blur`, `_InvFade` × 10.
    figures: vec4<f32>,
    // `_TextureMovement`.
    movement: vec4<f32>,
    // `_DistortTex_ST`.
    scale: vec4<f32>,
    // The camera's near plane, the most size on the screen (a share of the
    // view's height), and the projection's height scale.
    near: vec4<f32>,
}

@group(0) @binding(0) var<uniform> haze: Haze;
@group(0) @binding(1) var noise: texture_2d<f32>;
@group(0) @binding(2) var noise_sampler: sampler;
@group(0) @binding(3) var alpha_picture: texture_2d<f32>;
@group(0) @binding(4) var alpha_sampler: sampler;
@group(0) @binding(5) var scene_depth: texture_2d<f32>;

struct Corner {
    @location(0) position: vec3<f32>,
    @location(1) corner: vec2<f32>,
    @location(2) size: f32,
    @location(3) turn: f32,
    @location(4) color: vec4<f32>,
}

struct Out {
    @builtin(position) clip: vec4<f32>,
    // `_Distort` over its distance times red and green, `_Blur` (less a
    // little by its depth) times blue, alpha.
    @location(0) values: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) scrolled: vec4<f32>,
    @location(3) eye: f32,
}

@vertex
fn vertex(c: Corner) -> Out {
    let centre = haze.view_from_world * vec4<f32>(c.position, 1.0);
    // At most so large a share of the view (`m_MaxParticleSize`).
    let eye = max(-centre.z, 1e-4);
    let most = haze.near.y * 2.0 * eye / haze.near.z;
    let size = min(c.size, most);
    let s = sin(c.turn);
    let k = cos(c.turn);
    let along = c.corner * size;
    let offset = vec2<f32>(along.x * k - along.y * s, along.x * s + along.y * k);
    let view = vec4<f32>(centre.xy + offset, centre.z, 1.0);
    let clip = haze.clip_from_view * view;
    var out: Out;
    out.clip = clip;
    out.values = vec4<f32>(haze.figures.y / clip.w, haze.figures.y / clip.w, haze.figures.z - clip.z * 0.012, 1.0) * c.color;
    let uv = c.corner + vec2<f32>(0.5, 0.5);
    let scaled = uv * haze.scale.xy + haze.scale.zw;
    out.uv = scaled;
    out.scrolled = vec4<f32>(scaled, scaled) + haze.figures.x * haze.movement;
    out.eye = -view.z;
    return out;
}

@fragment
fn fragment(in: Out) -> @location(0) vec4<f32> {
    let scene = textureLoad(scene_depth, vec2<i32>(in.clip.xy), 0).r;
    let scene_eye = haze.near.x / max(scene, 1e-12);
    let soft = clamp((scene_eye - in.eye) * haze.figures.w, 0.0, 1.0);
    let moved = textureSample(noise, noise_sampler, in.scrolled.xy).xy + textureSample(noise, noise_sampler, in.scrolled.zw).xy - vec2<f32>(1.0);
    let alpha = textureSample(alpha_picture, alpha_sampler, in.uv).x * in.values.w;
    return soft * vec4<f32>(moved * in.values.xy, in.values.z, alpha) + vec4<f32>(0.5, 0.5, 0.0, 0.0);
}
