// Customs' clouds (`clouds_view.rs`; lighting spec, "Customs' clouds"):
// the game's cloud layer (`EFT.Rendering.Clouds`, its port of HDRP's cloud
// layer), its three passes in turn by the shader def given. Every way here
// is in the game's axes (the app's with z turned round).
//
// - BAKE_TEXTURE (`BakeCloudTexture`, once): each texel of the cloud
//   texture, the upper sky in lat-long, holds the light the layer scatters
//   toward the eye from the sun (x) and its opacity (y), the light marched
//   toward the sun through the layer's shell.
// - BAKE_SHADOWS (`BakeCloudShadows`, each frame): the sun's cookie, the
//   layer's opacity about the way to the sun, blended at its edges so that
//   it tiles; written across the other way (the app's light texture is
//   read so), each texel what the game's is at the other side.
// - DRAW (`Hidden/HDRP/Sky/CloudLayer`'s second pass, each frame): over
//   the sky alone (where the scene's depth is the far plane), in the
//   game's numbers, premultiplied.

const PI: f32 = 3.141593;
const HALF_PI: f32 = 1.570796;

// HDRP's quick arc tangent, its sign kept.
fn fast_atan(x: f32) -> f32 {
    let a = abs(x);
    let inside = a < 1.0;
    let t0 = select(1.0 / a, a, inside);
    let t1 = t0 * t0;
    let poly = (t1 * 0.087293 - 0.301895) * t1 + 1.0;
    let r = select(HALF_PI - poly * t0, poly * t0, inside);
    return select(r, -r, x < 0.0);
}

// The way's turn about up as the game's lat-long coordinates take it.
fn fast_atan2(x: f32, z: f32) -> f32 {
    let turn = select(-PI, PI, x >= 0.0);
    return fast_atan(x / z) + select(0.0, turn, z < 0.0);
}

// HDRP's quick arc cosine.
fn fast_acos(y: f32) -> f32 {
    let a = abs(y);
    let r = ((a * 0.046888 - 0.203471) * a + HALF_PI) * sqrt(1.0 - a);
    return select(PI - r, r, y >= 0.0);
}

// The upper sky's lat-long coordinates of a way (`GetLatLongCoords`).
fn lat_long(d: vec3<f32>) -> vec2<f32> {
    let u = fast_atan2(d.x, d.z) * 0.1591 + 0.5;
    let v = (HALF_PI - fast_acos(d.y)) * 0.3183 + 0.5;
    return vec2<f32>(u, v * 2.0 - 1.0);
}

// The way from a place at radius `r` to the sphere of radius `outer`
// along a way whose cosine with the place's up is `cos_chi`, the far root
// (`-IntersectSphere(...).x` with the cosine turned round, as the game
// asks): from inside the sphere, as every place here is.
fn to_sphere(outer: f32, cos_chi: f32, r: f32) -> f32 {
    let d = (outer / r) * (outer / r) - max(1.0 - cos_chi * cos_chi, 0.0);
    return select(r * (sqrt(d) - cos_chi), d, d < 0.0);
}

#ifdef BAKE_TEXTURE
struct Bake {
    // `_Params`: the way to the sun (xyz), the planet's radius (w).
    sun: vec4<f32>,
    // `_Params1`: the map's channels' opacities.
    opacities: vec4<f32>,
    // `_Params2`: the layer's turn (a share of a whole turn), the light's
    // steps, the extinction's figure, the altitude.
    layer: vec4<f32>,
    // `_Resolution`: one over the texture's width.
    resolution: vec4<f32>,
}

@group(0) @binding(0) var map: texture_2d<f32>;
@group(0) @binding(1) var map_sampler: sampler;
@group(0) @binding(2) var cloud_out: texture_storage_2d<rgba16float, write>;
@group(0) @binding(3) var<uniform> bake: Bake;

// The shell's layer is 800 m thick (`_Thickness`); the light's march goes
// at most 500 m.
const THICKNESS: f32 = 800.0;

fn weights() -> vec4<f32> {
    let o = bake.opacities;
    return o / max(o.r + o.g + o.b + o.a, 1.0);
}

fn map_at(uv: vec2<f32>) -> f32 {
    return dot(textureSampleLevel(map, map_sampler, uv, 0.0), weights());
}

// Where a way from the ground meets the layer's bottom and its range up to
// its top (`GetCloudVolumeIntersection`).
fn shell(d: vec3<f32>) -> vec2<f32> {
    let r = bake.sun.w;
    let start = to_sphere(r + bake.layer.w, d.y, r);
    let end = to_sphere(r + bake.layer.w + THICKNESS, d.y, r);
    return vec2<f32>(start, end - start);
}

// The layer's density at a place (`GetDensity`): its map's opacity there,
// a tenth of it inside the part of the shell that opacity fills.
fn density(p: vec3<f32>) -> f32 {
    let d = normalize(p);
    let thickness = map_at(lat_long(d));
    let s = shell(d);
    let middle = length(d * (s.x + 0.5 * s.y) - p);
    return select(0.1 * thickness, 0.0, s.y * thickness < middle);
}

// Henyey and Greenstein's phase.
fn phase(g: f32, cos_angle: f32) -> f32 {
    return (1.0 - g * g) / (4.0 * PI * pow(max(1.0 + g * g - 2.0 * g * cos_angle, 0.0), 1.5));
}

@compute @workgroup_size(8, 8, 1)
fn bake_texture(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(cloud_out);
    if (id.x >= size.x || id.y >= size.y) {
        return;
    }
    let res = bake.resolution.x;
    let uv = vec2<f32>(f32(id.x) * res - bake.layer.x, 2.0 * f32(id.y) * res);
    let opacity = map_at(uv);
    var out = vec2<f32>(opacity, opacity);
    let steps = i32(bake.layer.y);
    if (steps != 0) {
        out = vec2<f32>(0.0);
        if (opacity != 0.0) {
            // The sun turned about up by the layer's turn (`RotationUp`).
            let a = bake.layer.x * 6.283185;
            let cs = vec2<f32>(cos(a), sin(a));
            let s = bake.sun.xyz;
            let sun = vec3<f32>(s.x * cs.x - s.z * cs.y, s.y, s.x * cs.y + s.z * cs.x);
            // The texel's way (`InverseLatLong`, the upper sky).
            let lat = f32(id.y) * res * 3.14169;
            let lon = (uv.x - 0.5) * 6.285355;
            let y = sin(lat);
            let across = sqrt(1.0 - y * y);
            let dir = vec3<f32>(sin(lon) * across, y, cos(lon) * across);
            let c = dot(dir, sun);
            let phase0 = phase(0.7, c) + phase(-0.3, c);
            let phase1 = phase(0.525, c) + phase(-0.225, c);
            let range = shell(dir);
            let at = dir * (range.x + 0.5 * (1.0 - opacity) * range.y);
            let sigma_e = 0.1 * opacity * bake.layer.z;
            let through = exp(-sigma_e * range.y * opacity);
            // Toward the sun to the layer's top, at most 500 m, in steps.
            let r = bake.sun.w;
            let start = at + vec3<f32>(0.0, r, 0.0);
            let from_middle = length(start);
            let toward = clamp(to_sphere(r + bake.layer.w + THICKNESS, dot(start, sun) / from_middle, from_middle), 0.0, 500.0);
            let step = toward / f32(steps);
            var extinction = 0.0;
            for (var j = 1; j <= steps; j++) {
                extinction += density(at + sun * step * f32(j));
            }
            let e = -step * extinction * bake.layer.z;
            let luminance = exp(e) * phase0 + exp(e * 0.75) * phase1 * 0.75;
            out = vec2<f32>(luminance - luminance * through, 1.0 - through);
        }
    }
    textureStore(cloud_out, vec2<i32>(id.xy), vec4<f32>(out, 0.0, 0.0));
}
#endif

#ifdef BAKE_SHADOWS
struct Shadows {
    // `_FlowmapParam`: the wind's way across the layer (x, z), the scroll,
    // the layer's opacity.
    flow: vec4<f32>,
    // `_SunDirection` (xyz), the planet's radius (`_PlanetCenterRadius.w`).
    sun: vec4<f32>,
    // `_Params1`: the light's right (xyz), the altitude; `_Params3`: its up.
    right: vec4<f32>,
    up: vec4<f32>,
    // `_Params`: the shadow tint's luminance, the opacity's multiplier.
    shadow: vec4<f32>,
}

@group(0) @binding(0) var cloud: texture_2d<f32>;
@group(0) @binding(1) var cloud_sampler: sampler;
@group(0) @binding(2) var cookie_out: texture_storage_2d<rgba16float, write>;
@group(0) @binding(3) var<uniform> frame: Shadows;

// The layer's opacity along a way (`RenderClouds(dir).a`), scrolled by the
// wind.
fn opacity_along(dir: vec3<f32>) -> f32 {
    if (dir.y < 0.0) {
        return 0.0;
    }
    let r = frame.sun.w;
    let altitude = frame.right.w;
    let at = dir * to_sphere(r + altitude, dir.y, r);
    let reach = 2.0 * altitude;
    let alpha = fract(frame.flow.z / reach + vec2<f32>(0.0, 0.5)) - 0.5;
    let delta = vec3<f32>(frame.flow.x, 0.0, frame.flow.y) * reach;
    let one = textureSampleLevel(cloud, cloud_sampler, lat_long(normalize(at + alpha.x * delta)), 0.0).y;
    let two = textureSampleLevel(cloud, cloud_sampler, lat_long(normalize(at + alpha.y * delta)), 0.0).y;
    return mix(one, two, abs(2.0 * alpha.x)) * frame.flow.w;
}

fn shadow_at(uv: vec2<f32>) -> f32 {
    let dir = frame.sun.xyz + uv.x * 0.2 * frame.right.xyz - uv.y * 0.2 * frame.up.xyz;
    return opacity_along(normalize(dir));
}

fn smooth01(x: f32) -> f32 {
    return x * x * (3.0 - 2.0 * x);
}

@compute @workgroup_size(8, 8, 1)
fn bake_shadows(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(cookie_out);
    if (id.x >= size.x || id.y >= size.y) {
        return;
    }
    let res = 1.0 / f32(size.x);
    // The game's texel across is the other side's.
    let x = f32(size.x - 1u - id.x);
    let uv = vec2<f32>(x * res, 1.0 - f32(id.y) * res) * 2.0 - 1.0;
    var shadow = shadow_at(uv);
    // Blended with the far borders, so that it tiles.
    let uv2 = uv - 0.5;
    let w = clamp(uv2 / 0.5, vec2<f32>(0.0), vec2<f32>(1.0));
    let wx = smooth01(w.x);
    let wy = smooth01(w.y);
    if (uv2.x > 0.0) {
        shadow = mix(shadow, shadow_at(uv - vec2<f32>(2.0, 0.0)), wx);
    }
    if (uv2.y > 0.0) {
        var shadow2 = shadow_at(uv - vec2<f32>(0.0, 2.0));
        if (uv2.x > 0.0) {
            shadow2 = mix(shadow2, shadow_at(uv - vec2<f32>(2.0, 2.0)), wx);
        }
        shadow = mix(shadow, shadow2, wy);
    }
    let light = clamp(1.0 + frame.shadow.x - clamp(shadow * frame.shadow.y, 0.0, 1.0), 0.0, 1.0);
    textureStore(cookie_out, vec2<i32>(id.xy), vec4<f32>(light));
}
#endif

#ifdef DRAW
#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct Draw {
    // `_FlowmapParam`: the wind's way (x, z), the scroll, the opacity.
    flow: vec4<f32>,
    // `_Params1`: the sun's colour for the clouds (rgb), the altitude.
    sun_color: vec4<f32>,
    // The ambient for the clouds (rgb: the harmonics at straight down, to
    // gamma, at least nought, times the dimmer), the exposure multiplier.
    ambient: vec4<f32>,
    // The planet's radius (x).
    planet: vec4<f32>,
    // The world from the camera's clip, its place (the app's axes).
    world_from_clip: mat4x4<f32>,
    camera: vec4<f32>,
}

@group(0) @binding(0) var cloud: texture_2d<f32>;
@group(0) @binding(1) var cloud_sampler: sampler;
@group(0) @binding(2) var depth: texture_2d<f32>;
@group(0) @binding(3) var<uniform> draw: Draw;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let size = vec2<i32>(textureDimensions(depth));
    let pixel = clamp(vec2<i32>(in.position.xy), vec2<i32>(0), size - 1);
    // Only over the sky, as the game's depth test lets it at the far plane.
    if (textureLoad(depth, pixel, 0).r != 0.0) {
        discard;
    }
    let ndc = vec2<f32>(in.uv.x * 2.0 - 1.0, 1.0 - in.uv.y * 2.0);
    let near = draw.world_from_clip * vec4<f32>(ndc, 1.0, 1.0);
    let way = normalize(near.xyz / near.w - draw.camera.xyz);
    let dir = vec3<f32>(way.x, way.y, -way.z);
    if (dir.y < 0.0) {
        discard;
    }
    let r = draw.planet.x;
    let altitude = draw.sun_color.w;
    let at = dir * to_sphere(r + altitude, dir.y, r);
    let reach = 2.0 * altitude;
    let alpha = fract(draw.flow.z / reach + vec2<f32>(0.0, 0.5)) - 0.5;
    let delta = vec3<f32>(draw.flow.x, 0.0, draw.flow.y) * reach;
    let one = textureSampleLevel(cloud, cloud_sampler, lat_long(normalize(at + alpha.x * delta)), 0.0).xy;
    let two = textureSampleLevel(cloud, cloud_sampler, lat_long(normalize(at + alpha.y * delta)), 0.0).xy;
    let c = mix(one, two, abs(2.0 * alpha.x));
    var out = vec4<f32>(c.x * draw.sun_color.rgb + draw.ambient.rgb * c.y, c.y) * draw.flow.w;
    out = vec4<f32>(out.rgb * draw.ambient.w, out.a);
    return out;
}
#endif
