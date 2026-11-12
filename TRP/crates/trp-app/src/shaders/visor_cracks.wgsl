// A face shield's cracks drawn into the cracks' picture (`visor_view.rs`;
// inventory spec, "The face shield's mask", its cracks), as the game's
// compiled `Custom/Glass Scratch` draws each of `VisorEffect.Draw`'s quads:
// no matrix, a corner `p` at `(p.x + 0.09, p.y + 0.09, 0.09, 1.09)`, the
// picture read at `t = 2 R(uv - 0.4) + 0.4` (`R` the turn by `_Rotation`,
// the degrees given taken as radians), its alpha written to the target's
// one channel and added (`Blend One One`, the pipeline's). The quads'
// corners and turns come worked out (`trp_sim::visor::crack_quads`): for
// each, its lower left and upper right, then its turn.

struct Quads {
    // Two to a crack: (low.x, low.y, high.x, high.y), then (turn, 0, 0, 0).
    quads: array<vec4<f32>, 40>,
}

@group(0) @binding(0) var crack_tex: texture_2d<f32>;
@group(0) @binding(1) var crack_sampler: sampler;
@group(0) @binding(2) var<uniform> cracks: Quads;

struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) t: vec2<f32>,
}

// The quad's four corners as two triangles: (0, 0), (1, 0), (1, 1) and
// (0, 0), (1, 1), (0, 1) of its texture (the game's `GL.QUADS` order).
@vertex
fn vertex(@builtin(vertex_index) index: u32, @builtin(instance_index) crack: u32) -> Out {
    var corners = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u);
    let corner = corners[index];
    let uv = vec2<f32>(select(0.0, 1.0, corner == 1u || corner == 2u), select(0.0, 1.0, corner >= 2u));
    let quad = cracks.quads[2u * crack];
    let turn = cracks.quads[2u * crack + 1u].x;
    let p = mix(quad.xy, quad.zw, uv);
    var out: Out;
    out.position = vec4<f32>(p.x + 0.09, p.y + 0.09, 0.09, 1.09);
    let d = uv - 0.4;
    let turned = vec2<f32>(cos(turn) * d.x + sin(turn) * d.y, -sin(turn) * d.x + cos(turn) * d.y);
    out.t = 2.0 * turned + 0.4;
    return out;
}

@fragment
fn fragment(in: Out) -> @location(0) vec4<f32> {
    let a = textureSample(crack_tex, crack_sampler, in.t).a;
    return vec4<f32>(a, 0.0, 0.0, a);
}
