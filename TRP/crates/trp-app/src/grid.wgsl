// Grid lines on the test range's ground and boxes: the standard material's
// own shading with its base colour drawn toward a tone along lines that lie
// on whole steps of the world's own axes, so that every surface carries the
// same grid and a line on the ground meets its like on a wall.
//
// The lines are worked out per pixel from the world position, not looked up
// in a texture: each is drawn no thinner than a pixel, fades by how much of
// the pixel it would cover once it is thinner than that, and settles to an
// even tint where the cells themselves are smaller than a pixel, so the far
// ground neither crawls nor shows rings.

#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
}

struct GridLines {
    // The colour the lines are drawn toward.
    tone: vec4<f32>,
    // Three levels of line: metres between lines, metres wide, and how far
    // toward the tone each is drawn (0 leaves a level out).
    spacing: vec4<f32>,
    width: vec4<f32>,
    strength: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100)
var<uniform> grid: GridLines;

// How much of this pixel one level of lines covers, 0 to 1. `at` is the
// place on the surface along its two directions, in metres.
fn covered(at: vec2<f32>, spacing: f32, width: f32) -> f32 {
    let cell = at / spacing;
    // Cells a pixel spans along each direction.
    let pixel = fwidth(cell);
    let wanted = vec2<f32>(width / spacing);
    let drawn = clamp(wanted, pixel, vec2<f32>(0.5));
    let soft = pixel * 1.5;
    // Twice the distance to the nearest line, in cells: 0 on it, 1 midway.
    let off = 1.0 - abs(fract(cell) * 2.0 - 1.0);
    var cover = 1.0 - smoothstep(drawn - soft, drawn + soft, off);
    // A line widened to a pixel is that much fainter.
    cover *= clamp(wanted / drawn, vec2<f32>(0.0), vec2<f32>(1.0));
    // Cells smaller than a pixel: the share of the surface the lines take.
    cover = mix(cover, wanted, clamp(pixel * 2.0 - 1.0, vec2<f32>(0.0), vec2<f32>(1.0)));
    return mix(cover.x, 1.0, cover.y);
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    // The two world axes that lie in the surface: all but the one its
    // normal points most along.
    let place = in.world_position.xyz;
    let facing = abs(in.world_normal);
    var at = place.xz;
    if facing.x > facing.y && facing.x >= facing.z {
        at = place.zy;
    } else if facing.z > facing.y {
        at = place.xy;
    }

    let fine = covered(at, grid.spacing.x, grid.width.x) * grid.strength.x;
    let metre = covered(at, grid.spacing.y, grid.width.y) * grid.strength.y;
    let coarse = covered(at, grid.spacing.z, grid.width.z) * grid.strength.z;
    let lined = max(fine, max(metre, coarse));

    let base = pbr_input.material.base_color;
    pbr_input.material.base_color = vec4<f32>(mix(base.rgb, grid.tone.rgb, lined), base.a);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
