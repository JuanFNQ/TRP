// A weapon's muzzle flash (`muzzle_view.rs`; firing spec, "The muzzle
// flash"), as the game's compiled `Custom/Additive` does it: each quad's
// corners moved along and across its jet's axis by draws hashed from its
// colour and the shot's `_RandomVals`, then laid facing the camera at a
// size in the world, its picture cut by a window that sweeps the jet as the
// shot goes on, and added. Drawn on the frame in its gamma-encoded numbers,
// as the game adds them (`gamma_ui`), its depth tested against a copy of
// the scene's (`scene_depth.wgsl`).

struct Jet {
    clip_from_world: mat4x4<f32>,
    world_from_local: mat4x4<f32>,
    // The projection's own x and y scales.
    projection: vec4<f32>,
    // `_TintColor`.
    tint: vec4<f32>,
    // `_RotMx`.
    rot_mx: vec4<f32>,
    // `_RandomVals`.
    random_vals: vec4<f32>,
    // `_CellShift`, `_InvWindowSize`, `_RandomizePhase`.
    cell: vec4<f32>,
    // `_ShotVals`.
    shot: vec4<f32>,
    // `_ZShift`, `_HDR`.
    z_hdr: vec4<f32>,
}

@group(0) @binding(0) var<uniform> jet: Jet;
@group(0) @binding(1) var picture: texture_2d<f32>;
@group(0) @binding(2) var picture_sampler: sampler;
@group(0) @binding(3) var scene_depth: texture_2d<f32>;

struct Vertex {
    @location(0) position: vec3<f32>,
    // The jet's axis.
    @location(1) normal: vec3<f32>,
    // The atlas cell.
    @location(2) uv0: vec2<f32>,
    // The corner's direction times the particle's size.
    @location(3) uv1: vec2<f32>,
    @location(4) tangent: vec4<f32>,
    @location(5) color: vec4<f32>,
}

struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    // The tint, and the window as its w.
    @location(1) tint: vec4<f32>,
}

@vertex
fn vertex(v: Vertex) -> Out {
    let n = v.normal;
    let c = v.color;
    let t = v.tangent;
    let rv = jet.random_vals;
    // Across the axis, by the colour's first draw.
    var across = fract(c.r * rv.yzx) - 0.5;
    across = n.yzx * across.yzx - across * n.zxy;
    // Along it, by both draws.
    var draws = fract(vec2<f32>(c.g, c.r) * rv.z);
    var along = draws.y * t.x;
    draws = draws - 0.5;
    along = draws.x * t.w + along;
    // The window's place along the jet, which the shot moves.
    let phase = draws.y * jet.cell.w + c.b - jet.shot.y * 2.0 + 0.5;
    let window = abs(phase) * jet.cell.z + jet.shot.x;
    var p = n * along + v.position;
    p = across * t.y + p;
    // Across again, by the colour's second draw.
    var second = fract(c.g * rv.zwy) - 0.5;
    second = n.yzx * second.yzx - second * n.zxy;
    p = second * t.z + p;
    var clip = jet.clip_from_world * jet.world_from_local * vec4<f32>(p, 1.0);
    // The corner, turned by the shot's draw, at the projection's scale.
    let r = vec2<f32>(v.uv1.x * jet.rot_mx.x + v.uv1.y * jet.rot_mx.z, v.uv1.x * jet.rot_mx.y + v.uv1.y * jet.rot_mx.w);
    clip = vec4<f32>(clip.xy + r * jet.projection.xy, clip.z * jet.z_hdr.x, clip.w);
    var out: Out;
    out.clip = clip;
    out.uv = v.uv0 + jet.cell.xy;
    out.tint = vec4<f32>(jet.tint.rgb, window);
    return out;
}

@fragment
fn fragment(in: Out) -> @location(0) vec4<f32> {
    // Behind what the scene drew there: none (`ZTest LEqual`, the depth
    // reversed).
    let scene = textureLoad(scene_depth, vec2<i32>(in.clip.xy), 0).r;
    if (in.clip.z < scene) {
        discard;
    }
    let t = textureSample(picture, picture_sampler, in.uv);
    let c = t * (t.a - in.tint.w);
    if (all(c.rgb < vec3<f32>(0.01))) {
        discard;
    }
    return c * jet.z_hdr.y * in.tint;
}

