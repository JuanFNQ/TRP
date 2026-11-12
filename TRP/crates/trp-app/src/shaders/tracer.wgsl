// A tracer round's trace (`tracer_view`; ballistics spec, "A tracer"), as
// the game's compiled `Custom/TracerLight` draws it: each corner where the
// tracers' mesh puts it, its colour as given; the picture times the colour
// times `_Brightness`, added to the frame by its alpha (`Blend SrcAlpha
// One`), colour only, both faces, depth tested and not written.

struct Tracers {
    clip_from_world: mat4x4<f32>,
    // `_Brightness`, then nothing.
    brightness: vec4<f32>,
}

@group(0) @binding(0) var<uniform> tracers: Tracers;
@group(0) @binding(1) var picture: texture_2d<f32>;
@group(0) @binding(2) var picture_sampler: sampler;
@group(0) @binding(3) var scene_depth: texture_2d<f32>;

struct Corner {
    // Its place (the game's axes).
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
}

struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@vertex
fn vertex(c: Corner) -> Out {
    var out: Out;
    out.clip = tracers.clip_from_world * vec4<f32>(c.position.x, c.position.y, -c.position.z, 1.0);
    out.uv = c.uv;
    out.color = c.color;
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
    return in.color * tracers.brightness.x * textureSample(picture, picture_sampler, in.uv);
}
