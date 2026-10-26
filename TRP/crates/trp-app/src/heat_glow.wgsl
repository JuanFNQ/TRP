// A weapon part's glow with its heat (`heat_view.rs`; malfunctions spec,
// "The barrel's glow"), drawn over the part's own mesh and added onto it, as
// the weapons' surface shader adds it to what it emits with `USEHEAT` on:
// within an ellipsoid about the heat's centre, its half-axes the box's whole
// size, a strength of one less the distance's share times the heat's term,
// blended from the first colour toward the second by that strength and
// scaled by it, by the visible factor and by the reflection's weight.

#import bevy_pbr::{
    mesh_functions,
    forward_io::Vertex,
    view_transformations::position_world_to_clip,
    mesh_view_bindings::view,
}

struct Heat {
    // The centre (in the part's own axes) and the heat's term,
    // `((T - 30) * 0.1)^2`.
    centre_term: vec4<f32>,
    // The box's whole size, and `VisibleHeatAlpha`.
    size_visible: vec4<f32>,
    color_1: vec4<f32>,
    color_2: vec4<f32>,
    // `_SpecVals`' first two, and `HeatVisibleFactor`.
    spec: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> heat: Heat;

struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) local_position: vec3<f32>,
}

@vertex
fn vertex(vertex: Vertex) -> Out {
    var out: Out;
    // Placed as the mesh's own draw places it, so that the two meet.
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    out.world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
    out.position = position_world_to_clip(out.world_position.xyz);
#ifdef VERTEX_NORMALS
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex.instance_index);
#else
    out.world_normal = vec3(0.0, 1.0, 0.0);
#endif
    out.local_position = vertex.position;
    return out;
}

@fragment
fn fragment(in: Out) -> @location(0) vec4<f32> {
    let d = length((in.local_position - heat.centre_term.xyz) / heat.size_visible.xyz);
    let f = max(0.0, 1.0 - d) * heat.centre_term.w;
    // The reflection's weight, `0.5 * (x + y * 0.5 * (1 - n.v)^2)`: by the
    // mesh's own normal (the game's goes by its bumped one).
    let n = normalize(in.world_normal);
    let v = normalize(view.world_position - in.world_position.xyz);
    let rim = 1.0 - clamp(dot(n, v), 0.0, 1.0);
    let s = 0.5 * (heat.spec.x + heat.spec.y * 0.5 * rim * rim);
    let glow = mix(heat.color_1.rgb, heat.color_2.rgb, f) * f * heat.spec.z * s * heat.size_visible.w;
    // The game's frame is in gamma space and the glow is added to its
    // numbers (`PROVISIONAL`: here it is added in linear light, its numbers
    // decoded first, which is the game's sum over a dark surface, as a
    // barrel's is; not taken by the view's exposure, as it is not there).
    return vec4(linear_of(glow), 0.0);
}

// The sRGB curve undone: a colour's numbers as linear light.
fn linear_of(c: vec3<f32>) -> vec3<f32> {
    let v = max(c, vec3(0.0));
    return select(pow((v + 0.055) / 1.055, vec3(2.4)), v / 12.92, v <= vec3(0.04045));
}
