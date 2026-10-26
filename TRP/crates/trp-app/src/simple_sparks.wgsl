// The muzzles' sparks (`particles_view`; firing spec, "The muzzle's
// sparks"), as the game's compiled `Custom/SimpleParticleMoveAdd` draws a
// spark (with `TURBULENCE`): each corner of its streak at a time between
// the last frame's and this one's, by its share along the streak; there
// the spark's flight from where it was born (its velocity slowed by its
// drag to the power of its age, summed; gravity times its age squared
// taken off its height), two smooth waves of turbulence growing with its
// age, and the streak's width across its way on the screen. Bright by
// four times its share of life times one less it, times its emission;
// added to the frame (`Blend One One`), colour only, depth tested.

struct Sparks {
    clip_from_world: mat4x4<f32>,
    // `_MainTime`, `_LastMainTime`, `_Size`, `_FadeOut`.
    times: vec4<f32>,
    // `_TurbulenceFrequency0`, `_TurbulenceAmplitude0`,
    // `_TurbulenceFrequency1`, `_TurbulenceAmplitude1` (none without
    // `TURBULENCE`).
    turbulence: vec4<f32>,
    // `_TintColor`.
    tint: vec4<f32>,
}

@group(0) @binding(0) var<uniform> sparks: Sparks;
@group(0) @binding(1) var picture: texture_2d<f32>;
@group(0) @binding(2) var picture_sampler: sampler;
@group(0) @binding(3) var scene_depth: texture_2d<f32>;

struct Corner {
    // Where the spark was born (the game's axes).
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    // Along its streak (−1, 0, 1) and across (−1, 1).
    @location(2) offset: vec2<f32>,
    // Its share of the streak's time, and its slot's draw.
    @location(3) share_draw: vec2<f32>,
    // The time it was born at, its lifetime, its gravity, its drag.
    @location(4) born: vec4<f32>,
    // Its velocity (the game's axes).
    @location(5) velocity: vec3<f32>,
    // Its emission, size, turbulence and frequency, from 0 to 1.
    @location(6) bytes: vec4<f32>,
}

struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) bright: f32,
}

// How far it has gone at an age: its velocity summed under its drag, its
// gravity taken off its height.
fn gone(c: Corner, age: f32, ln: f32) -> vec3<f32> {
    var d = c.velocity * ((exp2(log2(c.born.w) * age) - 1.0) / ln);
    d.y = d.y - age * age * c.born.z;
    return d;
}

// A smooth triangle wave from −0.5 to 0.5.
fn wave(x: vec3<f32>) -> vec3<f32> {
    let w = abs(fract(x + vec3<f32>(0.5)) * 2.0 - vec3<f32>(1.0));
    return w * w * (vec3<f32>(3.0) - 2.0 * w) - vec3<f32>(0.5);
}

@vertex
fn vertex(c: Corner) -> Out {
    var out: Out;
    let frame = max(sparks.times.x - sparks.times.y, 0.033333);
    let at = sparks.times.x - (frame - c.share_draw.x * frame);
    let age = max(at - c.born.x, 0.0);
    let s = age / c.born.y;
    let life = s * (1.0 - s);
    if (life <= 0.0) {
        out.clip = vec4<f32>(0.0);
        out.uv = vec2<f32>(0.0);
        out.bright = 0.0;
        return out;
    }
    let ln = log2(c.born.w) * 0.693147;
    let here = gone(c, age, ln);
    let way = here - gone(c, age - 0.04, ln);
    var world = c.position + here;
    // The turbulence, growing with its age (`_FadeOut`).
    let grow = min(age * sparks.times.w, 1.0) * c.bytes.z;
    let frequency = c.bytes.w * vec2<f32>(sparks.turbulence.x, sparks.turbulence.z);
    let primes = vec3<f32>(769.0, 773.0, 787.0);
    let first = (fract((c.position.yzx + c.velocity.zxy) * primes) + vec3<f32>(0.2)) * (age + 17.0) * frequency.x + vec3<f32>(c.share_draw.y);
    world = world + wave(first) * sparks.turbulence.y * grow;
    let second = (fract((c.position.zxy + c.velocity.yzx) * primes) + vec3<f32>(0.2)) * (age + 17.0) * frequency.y + vec3<f32>(1.0 - c.share_draw.y);
    world = world + wave(second) * sparks.turbulence.w * grow;
    // Into the app's axes, then the streak across its way on the screen.
    let clip = sparks.clip_from_world * vec4<f32>(world.x, world.y, -world.z, 1.0);
    let toward = (sparks.clip_from_world * vec4<f32>(way.x, way.y, -way.z, 0.0)).xy;
    let along = toward * inverseSqrt(max(dot(toward, toward), 1e-20));
    let across = vec2<f32>(-along.y, along.x);
    let offset = (along * c.offset.x + across * c.offset.y) * sparks.times.z * c.bytes.y;
    out.clip = vec4<f32>(clip.xy + offset, clip.zw);
    out.uv = c.uv;
    out.bright = 4.0 * life * c.bytes.x;
    return out;
}

@fragment
fn fragment(in: Out) -> @location(0) vec4<f32> {
    // Behind what the scene drew there: none (`ZTest LEqual`, the depth
    // reversed).
    let scene = textureLoad(scene_depth, vec2<i32>(in.clip.xy), 0).r;
    if (in.clip.z < scene) {
        discard;
    }
    return textureSample(picture, picture_sampler, in.uv) * in.bright * sparks.tint;
}
