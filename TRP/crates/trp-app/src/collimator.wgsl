// A red dot's or a holographic sight's lens as the game draws it
// (`CW FX/Collimator`; optics spec, "The lens"; `optic_view.rs`): light
// added over the frame, the mark's alpha times its colour, `_HDR`, the
// noise's red over 1.3 and the fade's red. The mark's uv comes from the eye
// in the lens's own frame: with `d = eye - _MarkShift - v / _MarkScale`
// from the vertex `v` and `k = _MarkShift.y^2 / d.y`, `(0.5 + 0.5 k d.x,
// 0.5 - 0.5 k d.z)`: with the stock shift (0, -150, 0) the dot lies at
// infinity along the lens's y. The sum is made in the game's numbers and
// added as the frame's figures (`PROVISIONAL`: the game adds in its numbers
// to a frame in them; here the addend alone is turned).

#import bevy_pbr::mesh_functions
#import bevy_pbr::mesh_view_bindings::view
#import bevy_pbr::view_transformations::position_world_to_clip

struct Collimator {
    // `_Color.rgb`, `_HDR`.
    color: vec4<f32>,
    // `_MarkShift.xyz` (the game's axes), `_MarkScale`.
    mark: vec4<f32>,
    // The power of the game's numbers, whether there are a noise and a fade.
    space: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> collimator: Collimator;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var mark: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var mark_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var noise: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var noise_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var fade: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var fade_sampler: sampler;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(2) uv: vec2<f32>,
}

struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) mark_uv: vec2<f32>,
    @location(1) uv: vec2<f32>,
}

@vertex
fn vertex(in: Vertex) -> Out {
    var out: Out;
    let world_from_local = mesh_functions::get_world_from_local(in.instance_index);
    let local_from_world = mesh_functions::get_local_from_world(in.instance_index);
    out.clip = position_world_to_clip((world_from_local * vec4<f32>(in.position, 1.0)).xyz);
    // The eye and the vertex in the lens's frame, the game's axes.
    let e_app = (local_from_world * vec4<f32>(view.world_position, 1.0)).xyz;
    let e = vec3<f32>(e_app.x, e_app.y, -e_app.z);
    let v = vec3<f32>(in.position.x, in.position.y, -in.position.z);
    let shift = collimator.mark.xyz;
    let d = e - shift - v / collimator.mark.w;
    let k = shift.y * shift.y / d.y;
    out.mark_uv = vec2<f32>(0.5 + 0.5 * k * d.x, 0.5 - 0.5 * k * d.z);
    out.uv = in.uv;
    return out;
}

@fragment
fn fragment(in: Out) -> @location(0) vec4<f32> {
    let dot = textureSample(mark, mark_sampler, in.mark_uv).a;
    let grain = select(1.0, textureSample(noise, noise_sampler, in.uv * 1234.567749).r, collimator.space.y > 0.5);
    let edge = select(1.0, textureSample(fade, fade_sampler, in.uv).r, collimator.space.z > 0.5);
    let added = dot * collimator.color.rgb * grain / 1.3 * edge * collimator.color.w;
    return vec4<f32>(pow(max(added, vec3<f32>(0.0)), vec3<f32>(collimator.space.x)), 0.0);
}
