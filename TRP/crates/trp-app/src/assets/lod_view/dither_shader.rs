//! The dither shader and its patch into the mesh functions.

use super::*;

/// Bevy's mesh functions, where a mesh's dither level is had.
const MESH_FUNCTIONS: &str = "embedded://bevy_pbr/render/mesh_functions.wgsl";

const DITHER_LEVEL: &str = "fn get_visibility_range_dither_level(instance_index: u32, world_position: vec4<f32>) -> i32 {";

/// TRP's dither, as `trp_sim::lod::dither` has it: put before Bevy's
/// function, which hands a tagged mesh here.
pub(super) fn trp_lod_dither() -> String {
    format!(
        "
// (trp: TRP's own levels of detail, `lod_view`; `trp_sim::lod::dither`.)
// A piece's band rides in its tag's low 30 bits: the pixel sizes, metres, it
// is drawn between, as two half floats of 15 bits (their sign nought; the
// top two bits are the decals'). The view's own pixel at the piece says where
// in it the piece stands, so each view fades by its own resolution and
// field of view; an orthographic view's pixel is its texel at any distance.
fn trp_lod_dither(band_bits: u32, place: vec3<f32>) -> i32 {{
    let band = unpack2x16float((band_bits & 0x7fffu) | (((band_bits >> 15u) & 0x7fffu) << 16u));
    let fade = {fade:?};
    // A coarser level fades in a little before the finer fades out.
    let start = band.x * (1.0 - {overlap:?});
    var pixel = 2.0 / max(view.viewport.w * view.clip_from_view[1][1], 1e-9);
    if view.clip_from_view[3][3] != 1.0 {{
        pixel = pixel * length(view.world_position - place);
    }}
    if pixel < start {{
        return -16;
    }}
    if start > 0.0 && pixel < start * (1.0 + fade) {{
        return -16 + i32(round((pixel - start) / (start * fade) * 16.0));
    }}
    if pixel < band.y {{
        return 0;
    }}
    if pixel < band.y * (1.0 + fade) {{
        return i32(round((pixel - band.y) / (band.y * fade) * 16.0));
    }}
    return 16;
}}

",
        fade = trp_sim::characters::lod::FADE,
        overlap = trp_sim::characters::lod::OVERLAP
    )
}

/// What Bevy's function begins with once patched.
pub(super) const ASKED: &str = "
    // (trp: a tagged mesh's own band, `lod_view`; the tag's top two
    // bits are the decals'.)
    let trp_band = mesh[instance_index].tag & 0x3fffffffu;
    if trp_band != 0u {
        return trp_lod_dither(trp_band, world_position.xyz);
    }
";

#[derive(Resource)]
pub(super) struct Held(Handle<Shader>);

pub(super) fn hold(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(Held(assets.load(MESH_FUNCTIONS)));
}

/// Bevy's mesh functions, as they are loaded (and again if loaded afresh),
/// put back with TRP's dither; if the function is not found (another
/// Bevy), left as they are, with a line saying so: the levels then switch
/// at once in every view.
pub(super) fn patch(held: Option<Res<Held>>, mut events: MessageReader<AssetEvent<Shader>>, mut shaders: ResMut<Assets<Shader>>) {
    let Some(held) = held else { return };
    for event in events.read() {
        let (AssetEvent::Added { id } | AssetEvent::Modified { id } | AssetEvent::LoadedWithDependencies { id }) = *event else { continue };
        if id != held.0.id() {
            continue;
        }
        let Some(shader) = shaders.get(id) else { continue };
        let Source::Wgsl(source) = &shader.source else { continue };
        if source.contains("fn trp_lod_dither(") {
            continue;
        }
        let Some(function) = source.find(DITHER_LEVEL) else {
            warn!("{MESH_FUNCTIONS}: its dither level's function was not found; the levels of detail switch at once");
            continue;
        };
        let body = function + DITHER_LEVEL.len();
        let patched = format!("{}{}{}{ASKED}{}", &source[..function], trp_lod_dither(), &source[function..body], &source[body..]);
        let fresh = Shader::from_wgsl(patched, shader.path.clone());
        let known = shader.imports.clone();
        let replaced = Shader { source: fresh.source, imports: fresh.imports, import_path: fresh.import_path, ..shader.clone() };
        if let Some(new) = replaced.imports.iter().find(|import| !known.contains(import)) {
            warn!("{MESH_FUNCTIONS}: patched, it would import {new:?}, which it did not; the levels of detail switch at once");
            continue;
        }
        let _ = shaders.insert(id, replaced);
        info!("levels of detail: Bevy's dithered ranges patched to TRP's bands");
    }
}
