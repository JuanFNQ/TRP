// A mesh drawn as its shadow alone (`shadow_only.rs`): no pixel of it is
// kept in the camera's pass. The shadow pass does not run this shader, so
// the mesh's depth still goes into the sun's shadow map.

#import bevy_pbr::forward_io::{VertexOutput, FragmentOutput}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var out: FragmentOutput;
    discard;
    return out;
}
