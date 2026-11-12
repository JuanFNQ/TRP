//! Tests of the levels of detail by distance and TRP's own rule.
use super::*;

#[test]
fn a_body_parts_point_goes_through_the_mesh_transform() {
    // The bear body's group (`probe dump`: (-0.388, 0.213, 0.001), size
    // 2) under the player's `Mesh` (turned -90 degrees about x): its
    // point is at the feet's height, beside them.
    let group = trp_data::drawing::lod::LodGroup { id: None, object: None, size: 2.0, reference: [-0.388, 0.213, 0.001], fade_mode: 0, animate_cross_fading: false, levels: Vec::new(), deviation: vec![0.0, 0.0136] };
    let spec = LodSpec::through(&group, [0.0; 3], [-std::f32::consts::FRAC_1_SQRT_2, 0.0, 0.0, std::f32::consts::FRAC_1_SQRT_2]);
    let game = convert::position([-0.388, 0.001, -0.213]);
    assert!(spec.reference.distance(game) < 1e-5, "{:?} against {:?}", spec.reference, game);
    assert_eq!(spec.deviation, vec![0.0, 0.0136]);
}

#[test]
fn a_models_groups_switch_together() {
    let group = |deviation: Vec<f32>| trp_data::drawing::lod::LodGroup { id: None, object: None, size: 0.5, reference: [0.0; 3], fade_mode: 0, animate_cross_fading: false, levels: Vec::new(), deviation };
    let groups = [group(vec![0.0, 0.004]), group(vec![0.0, 0.017]), group(Vec::new())];
    let specs = LodSpec::bundle(groups.iter());
    assert_eq!(specs.iter().map(|spec| spec.deviation.clone()).collect::<Vec<_>>(), vec![vec![0.0, 0.017], vec![0.0, 0.017], vec![]]);
}

#[test]
fn the_shaders_dither_is_the_rules() {
    // The patch's function, with the fade's figure in it.
    let code = trp_lod_dither();
    assert!(code.contains(&format!("let fade = {:?};", trp_sim::characters::lod::FADE)), "{code}");
    assert!(code.contains(&format!("(1.0 - {:?})", trp_sim::characters::lod::OVERLAP)), "{code}");
    assert!(code.contains("fn trp_lod_dither(band_bits: u32, place: vec3<f32>) -> i32"));
    // The carrier never culls and is not abrupt (so Bevy's dithered
    // pipeline is taken).
    let range = carrier();
    assert!(!range.is_abrupt());
    for distance in [0.0, 1.0, 1e4, 1e9] {
        assert!(range.is_visible_at_all(distance));
    }
}

/// The patched function and the lines put into Bevy's are valid WGSL
/// (a word the language keeps, as `from`, fails every tagged mesh's
/// pipeline and none is drawn): parsed and validated by the engine's own
/// compiler beside a stand-in for the bindings they read.
#[test]
fn the_patch_is_valid_wgsl() {
    use wgpu::naga;
    let source = format!(
        "struct View {{ world_position: vec3<f32>, viewport: vec4<f32>, clip_from_view: mat4x4<f32> }}
struct Mesh {{ tag: u32 }}
var<private> view: View;
var<private> mesh: array<Mesh, 4>;
{}
fn get_level(instance_index: u32, world_position: vec4<f32>) -> i32 {{{ASKED}    return 0;
}}
@fragment
fn main() -> @location(0) vec4<f32> {{
    return vec4<f32>(f32(get_level(0u, vec4<f32>(0.0))));
}}",
        trp_lod_dither()
    );
    let module = naga::front::wgsl::parse_str(&source).unwrap_or_else(|error| panic!("{}", error.emit_to_string(&source)));
    naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all()).validate(&module).expect("valid");
}
