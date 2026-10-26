// A scope's lens as the game draws it (`CW FX/OpticSight`; optics spec,
// "The lens"; `optic_view.rs`). The game draws it in passes over a stencil:
// the lens mesh marks a window, then within it the optic camera's picture
// on the lens's disc scaled by `_Scales.x` and set on the plane `_Shifts.x`
// along the axis (ahead of the lens), the shadow on a funnel whose inner
// disc is shrunk by 0.99 `_Scales.y` and moved `_Shifts.y` toward the eye,
// and the reticle on the picture's plane. Here one pass on the lens mesh
// does the same: the ray from the eye through each of its pixels is met with
// those planes in the lens's own frame (its axis rescaled to the length of
// its x, `O'`, as the game's), exactly for the picture, the reticle and the
// funnel's inner disc; its annulus, the game's 24 flat quads, is taken as a
// cone (`PROVISIONAL`: the mask is below 0.06 there). The blends are done in
// the game's numbers (the frame's figures to the power `space.x`).

#import bevy_pbr::mesh_functions
#import bevy_pbr::mesh_view_bindings::view
#import bevy_pbr::view_transformations::position_world_to_clip

struct Lens {
    // `_Scales.xy`, `_Shifts.xy`.
    scales_shifts: vec4<f32>,
    // `_ShiftDirection` (the game's axes), `_NormalHideness`.
    direction: vec4<f32>,
    // The inner ring's radius, the rim's, `_MarkLightness`, 1 - `_SwitchToSight`.
    figures: vec4<f32>,
    // The power of the game's numbers, whether there is a reticle, the
    // picture's chromatic aberration (`ChromaticAberrationShift`), and
    // whether it is clear glass (`PIP` off: 1, and 2 with a reticle mesh's
    // picture).
    space: vec4<f32>,
    // With `PIP` off, the narrowing the lens is drawn squeezed across the
    // view by, about the eye (`optic_view::squeeze_drawn`), 1 otherwise; and
    // (yz) where the zeroing puts the reticle on the clear glass, right and
    // up from the picture's middle in its widths (optics spec, "With `PIP`
    // off").
    narrow: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> lens: Lens;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var picture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var picture_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var mark: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var mark_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var mask: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var mask_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(7) var mask2: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(8) var mask2_sampler: sampler;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
}

struct Out {
    @builtin(position) clip: vec4<f32>,
    // The vertex and the eye in the lens's frame `O'`, the game's axes.
    @location(0) local: vec3<f32>,
    @location(1) eye: vec3<f32>,
    // The reticle's fade with the eye's angle off the axis.
    @location(2) fade: f32,
}

// A direction of the drawn lens's with the squeeze across the view undone
// (`narrow.x`): the lens's frame as it is, the eye being the squeeze's
// middle (a vertex's and the eye's places in the lens's own axes are kept).
fn unsqueezed(c: vec3<f32>) -> vec3<f32> {
    let v = (view.view_from_world * vec4<f32>(c, 0.0)).xyz;
    return (view.world_from_view * vec4<f32>(v.xy * lens.narrow.x, v.z, 0.0)).xyz;
}

// With `PIP` off, the clear glass's black rim (the user's, 2026-10-24): its
// outer ring drawn this many times as far out, and set back from the eye by
// this share of its radius, so that what the see-through cut left open round
// the glass shows black, the scope's body in front of it (`ESTIMATE`: the
// app's own figures; seen from the eye about 7% past the glass, where the
// slivers reached 2.5% on the Specter and 1.3% on the Tango, whose housing
// ends 16% past it).
const RIM_OUT: f32 = 1.1;
const RIM_BACK: f32 = 0.2;

@vertex
fn vertex(in: Vertex) -> Out {
    var out: Out;
    let world_from_local = mesh_functions::get_world_from_local(in.instance_index);
    let local_from_world = mesh_functions::get_local_from_world(in.instance_index);
    // `O' = O diag(1, k, 1)`: the axis's column to the length of x's.
    let c0 = unsqueezed(world_from_local[0].xyz);
    let c1 = unsqueezed(world_from_local[1].xyz);
    let c2 = unsqueezed(world_from_local[2].xyz);
    let k = length(c0) / max(length(c1), 1e-8);
    var position = in.position;
    let rim = lens.figures.y;
    if (lens.space.w > 0.5 && lens.narrow.w > 0.5 && length(in.position.xz) > 0.5 * (lens.figures.x + rim)) {
        position = vec3<f32>(in.position.x * RIM_OUT, in.position.y - RIM_BACK * rim * k, in.position.z * RIM_OUT);
    }
    let world = world_from_local * vec4<f32>(position, 1.0);
    out.clip = position_world_to_clip(world.xyz);
    let e = (local_from_world * vec4<f32>(view.world_position, 1.0)).xyz;
    // Into the game's axes: z turned round.
    out.local = vec3<f32>(position.x, position.y / k, -position.z);
    out.eye = vec3<f32>(e.x, e.y / k, -e.z);
    // `pow(saturate(2 dot(O' _ShiftDirection, normalize(eye - origin)) - 1), _NormalHideness)`.
    let way = lens.direction.xyz;
    let axis = c0 * way.x + c1 * k * way.y - c2 * way.z;
    let toward = normalize(unsqueezed(view.world_position - world_from_local[3].xyz));
    out.fade = pow(saturate(2.0 * dot(axis, toward) - 1.0), lens.direction.w);
    return out;
}

fn to_game(c: vec3<f32>) -> vec3<f32> {
    return pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / lens.space.x));
}

fn from_game(c: vec3<f32>) -> vec3<f32> {
    return pow(max(c, vec3<f32>(0.0)), vec3<f32>(lens.space.x));
}

// One tap of the picture, `k` times as far from its middle as `d`, in the
// game's numbers (the camera's picture as a texture target holds it, its
// first row the top: the game's v is up from its bottom).
fn tap(k: f32, d: vec2<f32>) -> vec3<f32> {
    let at = vec2<f32>(0.5) + k * d;
    return to_game(textureSampleLevel(picture, picture_sampler, vec2<f32>(at.x, 1.0 - at.y), 0.0).rgb);
}

// The optic camera's chromatic aberration (`Hidden/ChromaticAberration`,
// pass 1, `_Shift` = (1 - s, 1 + s, 1 - s/2, 1 + s/2)), worked out where the
// lens reads the picture (`PROVISIONAL`: the game blits the whole picture
// once and the lens filters that; the same but for the filtering): red from
// the taps nearer the middle, blue from those farther, green from three, and
// the plain tap mixed back by (1 - 4 |d|^2)^2, whole at the middle.
fn aberrated(uv: vec2<f32>) -> vec3<f32> {
    let s = lens.space.z;
    let d = uv - vec2<f32>(0.5);
    let plain = tap(1.0, d);
    let a = tap(1.0 - s, d);
    let b = tap(1.0 + s, d);
    let c = tap(1.0 - 0.5 * s, d);
    let e = tap(1.0 + 0.5 * s, d);
    let shifted = vec3<f32>(0.666 * a.r + 0.333 * c.r, 0.333 * (plain.g + c.g + e.g), 0.666 * b.b + 0.333 * e.b);
    let f = 1.0 - 4.0 * dot(d, d);
    return mix(shifted, plain, f * f);
}

// Where the ray from `e` along `d` meets the plane at `h` along the axis,
// in that plane's own coordinates (x, z about the axis).
fn on_plane(e: vec3<f32>, d: vec3<f32>, h: f32) -> vec2<f32> {
    let way = lens.direction.xyz;
    let t = (h * way.y - e.y) / d.y;
    let q = e + d * t - h * way;
    return q.xz;
}

@fragment
fn fragment(in: Out) -> @location(0) vec4<f32> {
    let e = in.eye;
    let d = in.local - e;
    let inner = lens.figures.x;
    let rim = lens.figures.y;
    let shown = lens.figures.w;

    // The picture and the reticle: the disc scaled by `_Scales.x` on the
    // plane `_Shifts.x`, its uv `10 uv - 4.5` (the mesh's uv `0.5 + 0.4725
    // (x, z) / inner` within its inner ring).
    let p = on_plane(e, d, lens.scales_shifts.z) / lens.scales_shifts.x;
    let uv = vec2<f32>(0.5) + 4.725 / inner * p;
    let seen = aberrated(uv);
    let cut = textureSampleLevel(mask2, mask2_sampler, uv, 0.0).a;
    let reticle = textureSampleLevel(mark, mark_sampler, uv, 0.0) * lens.space.y;

    // The shadow: the funnel's inner disc (radius 0.99 `_Scales.y` times
    // the inner ring's, at `_Shifts.y` toward the eye), else its annulus to
    // the rim, else nothing (the annulus faces away and is culled).
    let h = lens.scales_shifts.w;
    let big = 0.99 * lens.scales_shifts.y * inner;
    let q = on_plane(e, d, h);
    var shadow_uv = vec2<f32>(0.5) + 0.4725 / big * q;
    var shadowed = true;
    if (length(q) > big) {
        shadowed = false;
        // The cone from radius `big` at `h` to `rim` at nought: its radius
        // at height y is `rim + (big - rim) y / h`.
        if (abs(h) > 1e-6) {
            let a_ = (big - rim) / h;
            let c0 = rim + a_ * e.y;
            let c1 = a_ * d.y;
            let a = d.x * d.x + d.z * d.z - c1 * c1;
            let b = 2.0 * (e.x * d.x + e.z * d.z - c0 * c1);
            let c = e.x * e.x + e.z * e.z - c0 * c0;
            let disc = b * b - 4.0 * a * c;
            if (disc >= 0.0 && abs(a) > 1e-12) {
                let r = sqrt(disc);
                let roots = array<f32, 2>((-b - r) / (2.0 * a), (-b + r) / (2.0 * a));
                var best = 1e30;
                for (var i = 0; i < 2; i++) {
                    let t = roots[i];
                    let y = e.y + d.y * t;
                    if (t > 0.0 && t < best && y >= min(0.0, h) && y <= max(0.0, h)) {
                        best = t;
                    }
                }
                if (best < 1e29) {
                    let x = e + d * best;
                    let along = 1.0 - x.y / h;
                    shadow_uv = vec2<f32>(0.5) + (0.4725 + 0.0275 * along) * normalize(x.xz);
                    shadowed = true;
                }
            }
        }
    }
    let shadow = select(1.0, textureSampleLevel(mask, mask_sampler, shadow_uv, 0.0).a, shadowed);

    // `lerp(picture * mask2 * s * shadow, (reticle + lightness) * fade * s, reticle.a * s)`.
    let base = seen * cut * shown * shadow;
    let lines = (to_game(reticle.rgb) + vec3<f32>(lens.figures.z)) * in.fade * shown;
    // With `PIP` off (`space.w`), clear glass: the reticle over the narrowed
    // world behind, which the soft circle and the shadow darken as they
    // darken the picture (optics spec, "With `PIP` off"; `PROVISIONAL`: the
    // game's masks on clear glass the app's own, as `PIP` off is), a black
    // rim past its disc; a variable scope's (`space.w` 2) its
    // reticle mesh's picture, its cover its alpha, read where the optic
    // camera's picture would be, under the mark. Black while its
    // see-through cut is not in force (`narrow.w` 0: the aim not settled).
    // Premultiplied: what is laid on, and the alpha takes the world behind.
    if (lens.space.w > 0.5) {
        if (lens.narrow.w < 0.5) {
            return vec4<f32>(0.0, 0.0, 0.0, 1.0);
        }
        // The rim: where the ray meets the lens's plane past its disc.
        if (length(on_plane(e, d, 0.0)) > rim) {
            return vec4<f32>(0.0, 0.0, 0.0, 1.0);
        }
        // The reticle moved to where the optic camera's turn to the zeroing
        // point would put the picture's middle.
        let moved = uv - lens.narrow.yz;
        let marked = textureSampleLevel(mark, mark_sampler, moved, 0.0) * lens.space.y;
        let over = marked.a * shown;
        let lines = (to_game(marked.rgb) + vec3<f32>(lens.figures.z)) * in.fade * shown;
        var colour = lines;
        var cover = over;
        if (lens.space.w > 1.5) {
            // Part of the picture: under the shadow and the soft circle.
            let meshed = textureSampleLevel(picture, picture_sampler, vec2<f32>(moved.x, 1.0 - moved.y), 0.0);
            let under = meshed.a * shown * cut;
            cover = over + under * (1.0 - over);
            colour = (lines * over + to_game(meshed.rgb) * shown * shadow * under * (1.0 - over)) / max(cover, 1e-5);
        }
        // The world behind as the picture would be: times the soft circle
        // and the shadow in the game's numbers, their power in the frame's
        // (the user's, 2026-10-24: "Shadow + soft circle").
        let keep = (1.0 - cover) * pow(cut * shadow, lens.space.x);
        return vec4<f32>(from_game(colour) * cover, 1.0 - keep);
    }
    let colour = mix(base, lines, reticle.a * shown);
    return vec4<f32>(from_game(colour), 1.0);
}
