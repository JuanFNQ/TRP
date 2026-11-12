// A weapon part's glow with its heat (`heat_view.rs`; malfunctions spec,
// "The barrel's glow"), as the weapons' surface shader adds it with
// `USEHEAT` on: within an ellipsoid about the heat's centre, its half-axes
// the box's whole size, a strength of one less the distance's share times
// the heat's term, blended from the first colour toward the second by that
// strength and scaled by it, by the visible factor and by the reflection's
// weight (by the bumped normal). Drawn over the part with the part's own
// material and shading, and added to that light as the game's numbers, as
// the game adds it to its emission target (exact under the game's shading,
// which sums by the 2.2 power; under Bevy's light, the frame's numbers by
// the same power, near the sRGB curve the frame is shown by: `PROVISIONAL`).

#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    decal::clustered::apply_decals,
    mesh_functions,
    mesh_view_bindings as view_bindings,
}

struct Heat {
    // The centre (in the part's own axes) and the heat's term,
    // `((T - 30) * 0.1)^2`.
    centre_term: vec4<f32>,
    // The box's whole size, and `VisibleHeatAlpha`.
    size_visible: vec4<f32>,
    color_1: vec4<f32>,
    color_2: vec4<f32>,
    // `_SpecVals`' first two, `HeatVisibleFactor`, and the part's alpha
    // cutoff (nought for an opaque part).
    spec: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> heat: Heat;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    // The place in the part's own axes.
    let local = (mesh_functions::get_local_from_world(in.instance_index) * vec4(in.world_position.xyz, 1.0)).xyz;
    let d = length((local - heat.centre_term.xyz) / heat.size_visible.xyz);
    let f = max(0.0, 1.0 - d) * heat.centre_term.w;
    // Nothing to add: the part's own draw stands.
    if f <= 0.0 || heat.size_visible.w <= 0.0 {
        discard;
    }

    var pbr_input = pbr_input_from_standard_material(in, is_front);
    if heat.spec.w > 0.0 && pbr_input.material.base_color.a < heat.spec.w {
        discard;
    }
    apply_decals(&pbr_input);

    // The reflection's weight, `0.5 * (x + y * 0.5 * (1 - n.v)^2)`, by the
    // bumped normal and the way to the eye.
    let rim = 1.0 - clamp(dot(pbr_input.N, pbr_input.V), 0.0, 1.0);
    let s = 0.5 * (heat.spec.x + heat.spec.y * 0.5 * rim * rim);
    let glow = mix(heat.color_1.rgb, heat.color_2.rgb, f) * f * heat.spec.z * s * heat.size_visible.w;

    // The part as lit, taken to the game's numbers (the frame is shown by
    // the 2.2 power: `gamma_ui`; the game's shading sums its light as those
    // numbers, `game_shading.wgsl`), the glow added there as the game adds
    // it to what the surface emits, and taken back.
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    let numbers = pow(max(out.color.rgb, vec3(0.0)), vec3(1.0 / 2.2)) + glow;
    // Laid over the part whole, as it would have been drawn.
    out.color = vec4(pow(numbers, vec3(2.2)), 1.0);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
