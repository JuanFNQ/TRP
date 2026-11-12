// The game's ambient highlight over a mesh of the hands' stencil type
// (`highlight_view.rs`; lighting spec, "The ambient highlight";
// `Hidden/AmbientHighlihgt`, disassembled): the time of day's harmonics
// without the sky's samples and the light from above, at the surface's
// normal (its normal map's, where it has one), taken to gamma
// (`LinearToGammaSpace`, held at nought), times the surface's colour and
// the multiplier; none within the indoor volume (one less its alpha).
// Added over the frame in linear light (its own figures made linear by the
// 2.2 power the frame is turned back into the game's numbers by).

#import bevy_pbr::forward_io::VertexOutput

struct Highlight {
    base_color: vec4<f32>,
    sh: array<vec4<f32>, 7>,
    // The multiplier, whether a normal map is given, the volume's alpha.
    figures: vec4<f32>,
    // The world into the indoor volume's unit box (Bevy's axes).
    indoors: mat4x4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> highlight: Highlight;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var base_picture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var base_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var normal_picture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var normal_sampler: sampler;

// The harmonics at a way, in the game's axes (`trp_sim::ambient::Harmonics::at`).
fn harmonics_at(n: vec3<f32>) -> vec3<f32> {
    let one = vec4<f32>(n, 1.0);
    let two = vec4<f32>(n.x * n.y, n.y * n.z, n.z * n.z, n.z * n.x);
    let c = highlight.sh[6].xyz * (n.x * n.x - n.y * n.y);
    return vec3<f32>(dot(highlight.sh[0], one) + dot(highlight.sh[3], two), dot(highlight.sh[1], one) + dot(highlight.sh[4], two), dot(highlight.sh[2], one) + dot(highlight.sh[5], two)) + c;
}

fn gamma(c: vec3<f32>) -> vec3<f32> {
    return max(1.055 * pow(max(c, vec3<f32>(0.0)), vec3<f32>(0.416667)) - vec3<f32>(0.055), vec3<f32>(0.0));
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let albedo = highlight.base_color.rgb * textureSample(base_picture, base_sampler, in.uv).rgb;
    var n = normalize(in.world_normal);
#ifdef VERTEX_TANGENTS
    if (highlight.figures.y > 0.0) {
        let rg = textureSample(normal_picture, normal_sampler, in.uv).rg * 2.0 - vec2<f32>(1.0);
        let z = sqrt(max(1.0 - dot(rg, rg), 0.0));
        let t = normalize(in.world_tangent.xyz - n * dot(in.world_tangent.xyz, n));
        let b = cross(n, t) * in.world_tangent.w;
        n = normalize(rg.x * t + rg.y * b + z * n);
    }
#endif
    // Within the indoor volume (its alpha), toward the light from straight
    // above by one less one less the alpha to the blur's power, and times
    // one less the alpha.
    var a = 0.0;
    let local = (highlight.indoors * vec4<f32>(in.world_position.xyz, 1.0)).xyz;
    if (all(abs(local) <= vec3<f32>(0.5))) {
        a = highlight.figures.z;
    }
    let full = gamma(harmonics_at(vec3<f32>(n.x, n.y, -n.z)));
    let top = gamma(vec3<f32>(highlight.sh[0].y + highlight.sh[0].w, highlight.sh[1].y + highlight.sh[1].w, highlight.sh[2].y + highlight.sh[2].w) - highlight.sh[6].xyz);
    let toward = 1.0 - pow(1.0 - a, highlight.figures.w);
    let light = mix(full, top, toward) * (1.0 - a) * highlight.figures.x;
    return vec4<f32>(albedo * pow(light, vec3<f32>(2.2)), 0.0);
}
