// The edit build screen's outline of a part (`preview_outline.rs`): the
// game's `Hidden/HighLightMesh`, its fifth pass. A pixel whose mask differs
// from the mask the line's width to its right, or the line's width above
// it, has the colour added to it once for each, on the frame's figures as
// the game's numbers (the 2.2 power) and back.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct Outline {
    // The line's width over the picture's width and height (x, y).
    offset: vec4<f32>,
    // The colour in the game's numbers; its alpha nought for none.
    color: vec4<f32>,
}

@group(0) @binding(0) var picture: texture_2d<f32>;
@group(0) @binding(1) var picture_sampler: sampler;
@group(0) @binding(2) var mask: texture_2d<f32>;
@group(0) @binding(3) var mask_sampler: sampler;
@group(0) @binding(4) var<uniform> outline: Outline;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let c = textureSample(picture, picture_sampler, in.uv);
    let here = textureSample(mask, mask_sampler, in.uv).r;
    let right = textureSample(mask, mask_sampler, in.uv + vec2<f32>(outline.offset.x, 0.0)).r;
    // (Up on the screen: the picture's coordinates run down.)
    let up = textureSample(mask, mask_sampler, in.uv - vec2<f32>(0.0, outline.offset.y)).r;
    let count = (select(0.0, 1.0, right != here) + select(0.0, 1.0, up != here)) * step(0.5, outline.color.a);
    let numbers = pow(max(c.rgb, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.2));
    let lit = numbers + count * outline.color.rgb;
    // (Where nothing was drawn, the line is drawn whole: the ground behind
    // the preview is the screen's, not the camera's.)
    return vec4<f32>(pow(lit, vec3<f32>(2.2)), max(c.a, min(count, 1.0)));
}
