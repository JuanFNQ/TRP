//! The game's ambient occlusion (`trp_sim::hbao`) beside recorder 0.11.0's
//! scene of Customs (course 0's second step, kept in
//! `local/traces/courses3/0-light-outside`): the camera's `HBAO` component
//! as the game ran it at the user's setting (`FastestPerformance`), and the
//! camera's far plane. The rest against the compiled passes' constants.
//! The recording's part is skipped where it is not there.

use trp_sim::hbao::{self, Blur, Quality, SsaoMode, FAR_PLANE};

fn recorded() -> Option<serde_json::Value> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../local/traces/courses3/0-light-outside/light-outside.scene.json");
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

#[test]
fn the_recorded_hbao_is_its_preset() {
    let Some(scene) = recorded() else {
        eprintln!("skipped: no recording of Customs' light (local/traces/courses3)");
        return;
    };
    let camera = &scene["camera"];
    assert_eq!(camera["far"].as_f64().unwrap() as f32, FAR_PLANE);
    let game = camera["components"].as_array().unwrap().iter().find(|c| c["type"] == "HBAO").expect("the recorded HBAO");
    assert_eq!(game["enabled"], true);
    let f = &game["fields"];
    let mode = SsaoMode::from_key(f["m_Presets"]["preset"].as_str().unwrap()).expect("a choice of the setting");
    assert_eq!(mode, SsaoMode::FastestPerformance);
    let ours = mode.hbao().expect("HBAO on");

    let general = &f["m_GeneralSettings"];
    let quality = match general["quality"].as_str().unwrap() {
        "Lowest" => Quality::Lowest,
        "Low" => Quality::Low,
        "Medium" => Quality::Medium,
        "High" => Quality::High,
        "Highest" => Quality::Highest,
        other => panic!("quality {other}"),
    };
    assert_eq!(ours.quality, quality);
    // What the app takes as every preset's own.
    assert_eq!(general["deinterleaving"], "Disabled");
    assert_eq!(general["resolution"], "Full");
    assert_eq!(general["noiseType"], "Dither");
    assert_eq!(general["displayMode"], "Normal");

    let ao = &f["m_AOSettings"];
    let number = |v: &serde_json::Value| v.as_f64().unwrap() as f32;
    assert_eq!(ours.radius, number(&ao["radius"]));
    assert_eq!(ours.max_radius_pixels, number(&ao["maxRadiusPixels"]));
    assert_eq!(ours.bias, number(&ao["bias"]));
    assert_eq!(ours.intensity, number(&ao["intensity"]));
    assert_eq!(ours.max_distance, number(&ao["maxDistance"]));
    assert_eq!(ours.distance_falloff, number(&ao["distanceFalloff"]));
    let base: Vec<f32> = ao["baseColor"].as_array().unwrap().iter().map(number).collect();
    assert_eq!(ours.base_color, [base[0], base[1], base[2]]);
    assert_eq!(ao["perPixelNormals"], "GBuffer");
    assert_eq!(ao["useMultiBounce"], false);
    assert_eq!(number(&ao["offscreenSamplesContribution"]), 0.0);

    assert_eq!(ours.color_bleeding, f["m_ColorBleedingSettings"]["enabled"].as_bool().unwrap());
    let blur = &f["m_BlurSettings"];
    let amount = match blur["amount"].as_str().unwrap() {
        "None" => Blur::None,
        "Narrow" => Blur::Narrow,
        "Medium" => Blur::Medium,
        "Wide" => Blur::Wide,
        "ExtraWide" => Blur::ExtraWide,
        other => panic!("blur {other}"),
    };
    assert_eq!(ours.blur, amount);
    assert_eq!(ours.sharpness, number(&blur["sharpness"]));
    assert_eq!(ours.blur_downsample, blur["downsample"].as_bool().unwrap());
    assert_eq!(f["noiseTex"]["size"], serde_json::json!([4, 4]));

    // The cinematic effect, off at this choice.
    let cinematic = camera["components"].as_array().unwrap().iter().find(|c| c["type"] == "UnityStandardAssets.CinematicEffects.AmbientOcclusion").expect("the recorded cinematic AO");
    assert_eq!(cinematic["enabled"].as_bool().unwrap(), mode.cinematic());
}

#[test]
fn the_choices_read_as_the_game_writes_them() {
    for mode in SsaoMode::ALL {
        assert_eq!(SsaoMode::from_key(mode.key()), Some(mode));
    }
    assert_eq!(SsaoMode::from_key("fastestperformance"), Some(SsaoMode::FastestPerformance));
    assert_eq!(SsaoMode::Off.hbao(), None);
    assert!(!SsaoMode::FastPerformance.cinematic());
    assert!(SsaoMode::HighQuality.cinematic());
}

/// The passes' counts and constants as `Hidden/HBAO` has them compiled.
#[test]
fn the_passes_take_the_compiled_constants() {
    let counts: Vec<(u32, u32)> = [Quality::Lowest, Quality::Low, Quality::Medium, Quality::High, Quality::Highest].iter().map(|q| (q.directions(), q.steps())).collect();
    assert_eq!(counts, [(3, 2), (4, 3), (6, 4), (8, 4), (8, 6)]);
    // Each pass's final share, 1 / (directions × steps): 0.166667, 0.083333, 0.031250, 0.020833.
    for (q, share) in [(Quality::Lowest, 0.166667), (Quality::Low, 0.083333), (Quality::High, 0.031250), (Quality::Highest, 0.020833)] {
        assert!((1.0 / (q.directions() * q.steps()) as f32 - share).abs() < 1e-6);
    }
    for (blur, falloff) in [(Blur::Narrow, 0.5), (Blur::Medium, 0.222222), (Blur::Wide, 0.125), (Blur::ExtraWide, 0.08)] {
        assert!((blur.falloff() - falloff).abs() < 1e-6, "{blur:?}");
    }
    let turns = hbao::turns(Quality::Lowest);
    assert!((turns[1][0] + 0.5).abs() < 1e-6 && (turns[1][1] - 0.866025).abs() < 1e-6);
    assert!((turns[2][0] + 0.5).abs() < 1e-6 && (turns[2][1] + 0.866025).abs() < 1e-6);
    let eight = hbao::turns(Quality::High);
    assert!((eight[1][0] - 0.707107).abs() < 1e-6 && (eight[1][1] - 0.707107).abs() < 1e-6);
}

#[test]
fn the_noise_keeps_a_negative_cosine_as_nought() {
    let noise = hbao::noise(Quality::Lowest);
    // The bottom row's first texel: the table's first two numbers, a turn
    // of 2π × 0.463937 / 3.
    let turn = std::f64::consts::PI * 2.0 * 0.463937 / 3.0;
    assert_eq!(noise[0], [(turn.cos() * 255.0).round() as u8, (turn.sin() * 255.0).round() as u8, (0.340042f64 * 255.0).round() as u8, 255]);
    // The second column's bottom texel: the ninth number, 0.778313, a turn
    // past a quarter.
    assert_eq!(noise[1][0], 0);
    assert!(noise[1][1] > 200);
    assert!(noise.iter().all(|t| t[3] == 255));
}

#[test]
fn the_depth_keeps_its_share_of_the_far_plane() {
    for z in [0.03, 1.0, 12.5, 149.0] {
        let rg = hbao::pack_depth(z);
        // Stored in bytes, as the game's texture is.
        let stored = rg.map(|c| (c * 255.0).round() / 255.0);
        assert!((hbao::unpack_depth(stored) * FAR_PLANE - z).abs() < FAR_PLANE / 255.0 / 255.0 + 1e-3, "{z}");
    }
    // One step of the stored depth, as the blur weighs it at sharpness 8.
    let step = 1.0 / 255.0 / 255.0;
    let w = hbao::blur_weight(Blur::ExtraWide, 0, step, 8.0);
    assert!((w - (-(step * FAR_PLANE * 8.0).powi(2)).exp2()).abs() < 1e-6);
    assert_eq!(hbao::composite(1.0, 0.75, [0.0; 3]), [1.0; 3]);
    assert_eq!(hbao::composite(0.0, 0.75, [0.0; 3]), [0.0; 3]);
}

#[test]
fn the_figures_follow_the_view() {
    let s = SsaoMode::FastestPerformance.hbao().unwrap();
    let tan = (74.0f32.to_radians() * 0.5).tan();
    let f = hbao::Figures::of(&s, 1440, tan);
    assert!((f.radius - 0.5 * 0.5 * 1440.0 / (2.0 * tan)).abs() < 1e-3);
    assert!((f.ao_multiplier - 2.0 / 0.95).abs() < 1e-6);
    assert_eq!(f.neg_inv_radius2, -4.0);
    let uv = hbao::uv_to_view(tan, 2560.0 / 1440.0);
    assert!((uv[0] - 2.0 * tan * 2560.0 / 1440.0).abs() < 1e-5 && (uv[1] + 2.0 * tan).abs() < 1e-6);
}
