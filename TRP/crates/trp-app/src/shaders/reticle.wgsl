// A variable scope's reticle mesh, drawn into the optic camera's picture as
// the game draws it (`Custom/Crossfair`, `OpticRetrice`; optics spec, "The
// lens"; `optic_view.rs`): placed by the optic camera's projection, then
// its clip-space x and y scaled by `_Scale` (the stored scale times 0.1)
// about the picture's middle; its colour the vertex's times `_Color`, put
// over what is drawn whatever the depth, triangle after triangle.

#import bevy_pbr::mesh_functions
#import bevy_pbr::view_transformations::position_world_to_clip

struct Reticle {
    // `_Color`.
    color: vec4<f32>,
    // `_Scale`, the power of the game's numbers.
    figures: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> reticle: Reticle;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(5) color: vec4<f32>,
}

struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vertex(in: Vertex) -> Out {
    var out: Out;
    let world_from_local = mesh_functions::get_world_from_local(in.instance_index);
    var clip = position_world_to_clip((world_from_local * vec4<f32>(in.position, 1.0)).xyz);
    clip.x = clip.x * reticle.figures.x;
    clip.y = clip.y * reticle.figures.x;
    out.clip = clip;
    out.color = in.color * reticle.color;
    return out;
}

@fragment
fn fragment(in: Out) -> @location(0) vec4<f32> {
    return vec4<f32>(pow(max(in.color.rgb, vec3<f32>(0.0)), vec3<f32>(reticle.figures.y)), 1.0);
}
