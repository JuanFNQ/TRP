// The prepass of a mesh drawn as its shadow alone (`shadow_only.rs`). In the
// camera's normal prepass (the ambient occlusion's, `hbao_view`) no pixel of
// it is kept, as none is in the camera's own pass, so neither its depth nor
// its normals hide what is behind it. In a light's shadow pass, which runs a
// prepass shader only for a masked material or to write a directional
// light's depth where the card cannot clamp it, it does what Bevy's own
// prepass does, and the mesh's shadow is drawn as before.

#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_prepass_functions,
}

#ifdef PREPASS_FRAGMENT
@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
#ifdef NORMAL_PREPASS
    discard;
#endif
    pbr_prepass_functions::prepass_alpha_discard(in);
    var out: FragmentOutput;
#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.frag_depth = in.unclipped_depth;
#endif
    return out;
}
#else
@fragment
fn fragment(in: VertexOutput) {
    pbr_prepass_functions::prepass_alpha_discard(in);
}
#endif
