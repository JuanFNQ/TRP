// The prepass of the dithered glass (`glass_view.rs`): the same pixels
// thrown away as in the camera's own pass (`glass.wgsl`), so that neither
// the depth nor the normals of a thrown pixel hide what is behind it (the
// camera's normal prepass, the ambient occlusion's: `hbao_view`); a kept
// pixel's normal as Bevy's own prepass writes it.

#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_prepass_functions,
    pbr_bindings,
    pbr_types,
    pbr_functions,
    mesh_bindings::mesh,
    mesh_view_bindings::view,
}
#import bevy_render::bindless::{bindless_samplers_filtering, bindless_textures_2d}
#ifdef BINDLESS
#import bevy_pbr::pbr_bindings::material_indices
#endif

struct Glass {
    color: vec4<f32>,
    spec_color: vec4<f32>,
    reflect_color: vec4<f32>,
    figures: vec4<f32>,
    ambient: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> glass: Glass;

// As `glass.wgsl`'s.
fn thrown(frag: vec2<f32>) -> bool {
    let p = vec2<i32>(floor(frag - view.viewport.xy));
    let y = i32(view.viewport.w) - 1 - p.y;
    return ((p.x + y) & 1) == 1;
}

// The glass's alpha: the colour picture's times `_Color`'s times
// `_OpacityScale`, held from nought to one.
fn glass_alpha(in: VertexOutput) -> f32 {
    var a = 1.0;
#ifdef VERTEX_UVS_A
#ifdef BINDLESS
    let slot = mesh[in.instance_index].material_and_lightmap_bind_group_slot & 0xffffu;
    a = textureSampleBias(bindless_textures_2d[material_indices[slot].base_color_texture], bindless_samplers_filtering[material_indices[slot].base_color_sampler], in.uv, view.mip_bias).a;
#else
    a = textureSampleBias(pbr_bindings::base_color_texture, pbr_bindings::base_color_sampler, in.uv, view.mip_bias).a;
#endif
#endif
    return saturate(a * glass.color.a * glass.figures.x);
}

#ifdef PREPASS_FRAGMENT
@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    if glass_alpha(in) < 0.5 && thrown(in.position.xy) {
        discard;
    }
    var out: FragmentOutput;
#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.frag_depth = in.unclipped_depth;
#endif
#ifdef NORMAL_PREPASS
#ifdef BINDLESS
    let slot = mesh[in.instance_index].material_and_lightmap_bind_group_slot & 0xffffu;
    let flags = pbr_bindings::material_array[material_indices[slot].material].flags;
#else
    let flags = pbr_bindings::material.flags;
#endif
    let double_sided = (flags & pbr_types::STANDARD_MATERIAL_FLAGS_DOUBLE_SIDED_BIT) != 0u;
    var normal = pbr_functions::prepare_world_normal(in.world_normal, double_sided, is_front);
#ifdef VERTEX_UVS_A
#ifdef VERTEX_TANGENTS
#ifdef STANDARD_MATERIAL_NORMAL_MAP
#ifdef BINDLESS
    let Nt = textureSampleBias(bindless_textures_2d[material_indices[slot].normal_map_texture], bindless_samplers_filtering[material_indices[slot].normal_map_sampler], in.uv, view.mip_bias).rgb;
#else
    let Nt = textureSampleBias(pbr_bindings::normal_map_texture, pbr_bindings::normal_map_sampler, in.uv, view.mip_bias).rgb;
#endif
    let TBN = pbr_functions::calculate_tbn_mikktspace(normal, in.world_tangent);
    normal = pbr_functions::apply_normal_mapping(flags, TBN, double_sided, is_front, Nt);
#endif
#endif
#endif
    out.normal = vec4(normal * 0.5 + vec3(0.5), 1.0);
#endif
#ifdef MOTION_VECTOR_PREPASS
    out.motion_vector = pbr_prepass_functions::calculate_motion_vector(in.world_position, in.previous_world_position);
#endif
    return out;
}
#else
@fragment
fn fragment(in: VertexOutput) {
    if glass_alpha(in) < 0.5 && thrown(in.position.xy) {
        discard;
    }
}
#endif
