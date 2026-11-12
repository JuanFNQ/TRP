// The camera's undithering (`Hidden/Undithering` as the main camera runs
// it, its distortion off: `glass_view.rs`; body-animator spec, "The shield's
// glass"), on the frame in the game's numbers after the ambient occlusion
// and the sky's scattering, before anything transparent of the app's. A
// pixel is the dithered glass's where the frame's alpha marks it (under a
// half: `glass.wgsl` writes nought) and the scene is not the sky there (its
// depth not the far plane's, nought); wherever a pixel or the one a row
// over is so marked, both are taken as their mean, alpha too, a mark's
// alpha taken as whole. On a preview's camera (`FRAME_LINEAR`: the frame's
// figures linear by the 2.2 power, its photographic curve after this) the
// figures are taken to the game's numbers for the mean and back.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var picture: texture_2d<f32>;
@group(0) @binding(1) var depth: texture_2d<f32>;

fn marked(c: vec4<f32>, d: f32) -> bool {
    return c.a < 0.5 && d > 0.0;
}

// A colour of the frame as the game's numbers, and back.
fn numbers(c: vec3<f32>) -> vec3<f32> {
#ifdef FRAME_LINEAR
    return pow(max(c, vec3(0.0)), vec3(1.0 / 2.2));
#else
    return c;
#endif
}

fn figures(c: vec3<f32>) -> vec3<f32> {
#ifdef FRAME_LINEAR
    return pow(max(c, vec3(0.0)), vec3(2.2));
#else
    return c;
#endif
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let p = vec2<i32>(floor(in.position.xy));
    // The row over: the one above as seen (the engine's v runs up the
    // picture).
    let q = vec2<i32>(p.x, max(p.y - 1, 0));
    let cp = textureLoad(picture, p, 0);
    let cq = textureLoad(picture, q, 0);
    let mp = marked(cp, textureLoad(depth, p, 0).r);
    let mq = marked(cq, textureLoad(depth, q, 0).r);
    if !(mp || mq) {
        return cp;
    }
    let ap = select(cp.a, 1.0, mp);
    let aq = select(cq.a, 1.0, mq);
    return vec4(figures((numbers(cp.rgb) + numbers(cq.rgb)) * 0.5), (ap + aq) * 0.5);
}
