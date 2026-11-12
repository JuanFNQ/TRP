// PROVISIONAL: a drop's clip-space y +1 is taken to the picture's first row
// and that row to the screen's foot (the engine's flip of a render texture
// not read); the 2048 square of the frame is in the frame's own format.
// Blood on the screen (`blood_view.rs`; health spec, "Blood on the screen"),
// as the game's compiled shaders do it: `Blood/BloodDrop` (a drop into the
// drops' picture), `Hidden/Blur` (seven taps), `Blood/BloodBlit` (the drops
// laid into the distortion map), `Blood/BloodCountdown` (the map's fade),
// `Hidden/BloodOnScreen` (the frame seen through it) and
// `Hidden/CC_FastVignette`'s pass 1 (a heavy bleeding's red pulse). The
// distortion map keeps the game's rows: its first row the screen's foot.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct Blood {
    // The drops drawn this frame: place, scale, life.
    drops: array<vec4<f32>, 8>,
    count: f32,
    aspect: f32,
    // The fade's share this frame (`_FadeTime`).
    fade: f32,
    // The blur's step in the drops' picture (`_BlurOffsets0`).
    step: f32,
    refraction: f32,
    blurred: f32,
    heavy: f32,
    on: f32,
    side: f32,
    passes: f32,
    input_min: vec4<f32>,
    input_max: vec4<f32>,
    input_gamma: vec4<f32>,
    output_min: vec4<f32>,
    output_max: vec4<f32>,
    color: vec4<f32>,
    // The pulse's `_Data`: centre, sharpness × 0.01, darkness × 0.02.
    vignette: vec4<f32>,
}

@group(0) @binding(0) var picture: texture_2d<f32>;
@group(0) @binding(1) var linear_sampler: sampler;
@group(0) @binding(2) var<uniform> blood: Blood;
@group(0) @binding(3) var second: texture_2d<f32>;
@group(0) @binding(4) var third: texture_2d<f32>;

struct DropOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) life: f32,
}

// A drop's square about its place, in the picture's clip space: its place's
// y turned over and the whole y times the camera's shape (`BloodDrop.Draw`).
@vertex
fn drop_vertex(@builtin(vertex_index) vertex: u32, @builtin(instance_index) instance: u32) -> DropOut {
    let d = blood.drops[instance];
    var corners = array<vec2<f32>, 6>(vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(1.0, 1.0), vec2(0.0, 0.0), vec2(1.0, 1.0), vec2(0.0, 1.0));
    let uv = corners[vertex];
    var p = (uv * 2.0 - 1.0) * d.z + vec2<f32>(d.x, -d.y);
    p.y = p.y * blood.aspect;
    var out: DropOut;
    out.position = vec4<f32>(p, 0.0, 1.0);
    out.uv = uv;
    out.life = d.w;
    return out;
}

@fragment
fn drop_fragment(in: DropOut) -> @location(0) vec4<f32> {
    let t = textureSample(picture, linear_sampler, in.uv);
    let shown = ceil(t.a - 0.01);
    return vec4<f32>(t.rg * shown, clamp(t.a * in.life, 0.0, 1.0), t.a);
}

fn blur(uv: vec2<f32>, offset: vec2<f32>) -> vec4<f32> {
    var sum = textureSample(picture, linear_sampler, uv + offset) * 0.22;
    sum = textureSample(picture, linear_sampler, uv) * 0.28 + sum;
    sum = textureSample(picture, linear_sampler, uv - offset) * 0.22 + sum;
    sum = textureSample(picture, linear_sampler, uv + offset * 2.0) * 0.1 + sum;
    sum = textureSample(picture, linear_sampler, uv - offset * 2.0) * 0.1 + sum;
    sum = textureSample(picture, linear_sampler, uv + offset * 3.0) * 0.04 + sum;
    sum = textureSample(picture, linear_sampler, uv - offset * 3.0) * 0.04 + sum;
    return sum;
}

@fragment
fn blur_across(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    return blur(in.uv, vec2<f32>(blood.step, 0.0));
}

@fragment
fn blur_down(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    return blur(in.uv, vec2<f32>(0.0, blood.step));
}

// The drops (`picture`) laid over the map (`second`) by their blue, each
// first taken by it but for the blue.
@fragment
fn blit(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let m = textureSample(picture, linear_sampler, in.uv);
    let laid = vec4<f32>(m.r * m.b, m.g * m.b, m.b, m.a * m.b);
    let under = textureSample(second, linear_sampler, in.uv);
    return m.b * (laid - under) + under;
}

// The map's blue less the fade where it is above it, else nought.
@fragment
fn countdown(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let fade = max(blood.fade, 0.003906);
    var t = textureSample(picture, linear_sampler, in.uv);
    t.b = t.b - fade;
    return ceil(t.b) * t;
}

@fragment
fn copy(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    return textureSample(picture, linear_sampler, in.uv);
}

// The frame (`picture`) seen through the map (`second`, by the game's rows)
// and the blurred frame (`third`), then toward the blood's colour.
@fragment
fn blood_on_screen(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let map = textureSample(second, linear_sampler, vec2<f32>(in.uv.x, 1.0 - in.uv.y));
    let thick = clamp(map.b + map.b, 0.0, 1.0);
    let offset = map.rg * blood.refraction * thick;
    let through = vec2<f32>(in.uv.x + offset.x, in.uv.y - offset.y);
    let soft = textureSample(third, linear_sampler, through);
    let level = min(max(soft - blood.input_min, vec4<f32>(0.0)) / (blood.input_max - blood.input_min), vec4<f32>(1.0));
    let leveled = pow(level, 1.0 / blood.input_gamma) * (blood.output_max - blood.output_min) + blood.output_min;
    let frame = textureSample(picture, linear_sampler, in.uv);
    let seen = min(length(offset) * 50.0, 1.0) * (leveled - frame) + frame;
    let k = min(map.a, map.r) * thick;
    return vec4<f32>(k * (blood.color.rgb - seen.rgb) + seen.rgb, k * (k - seen.a) + seen.a);
}

// `Hidden/CC_FastVignette`'s pass 1, its colour red: the picture darkened
// toward the edges, its grey there tinted by the colour.
@fragment
fn heavy_pulse(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let uv = vec2<f32>(in.uv.x, 1.0 - in.uv.y);
    let d = length(uv - blood.vignette.xy);
    let t = clamp((d * (blood.vignette.z + blood.vignette.w) - 0.8) / (blood.vignette.z * 0.799 - 0.8), 0.0, 1.0);
    let s = t * t * (3.0 - 2.0 * t);
    let c = textureSample(picture, linear_sampler, in.uv);
    let dark = c.rgb * s;
    let grey = dot(dark, vec3<f32>(0.3, 0.59, 0.11));
    let red = vec4<f32>(1.0, 0.0, 0.0, 1.0);
    let tinted = vec4<f32>(grey, grey, grey, c.a) * red;
    return s * (vec4<f32>(dark, c.a) - tinted) + tinted;
}
