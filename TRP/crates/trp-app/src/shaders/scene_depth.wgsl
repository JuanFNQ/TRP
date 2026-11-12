// The scene's depth, its first sample, copied into a picture a later pass
// can read (`muzzle_view.rs`): the camera's own is multisampled, and only a
// depth attachment of the same samples could test against it.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

#ifdef MULTISAMPLED
@group(0) @binding(0) var depth: texture_depth_multisampled_2d;
#else
@group(0) @binding(0) var depth: texture_depth_2d;
#endif

@fragment
fn copy_depth(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(textureLoad(depth, vec2<i32>(in.position.xy), 0), 0.0, 0.0, 0.0);
}
