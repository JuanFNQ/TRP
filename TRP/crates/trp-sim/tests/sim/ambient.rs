//! Factory by day's light as the install sets it up, beside recorder
//! 0.11.0's scene of Factory by day (course 0's first step, kept in
//! `local/traces/courses3/0-light-factory`): the sun's turn, colour,
//! intensity and shadows, the level settings' ambient and fog, and the
//! ambient harmonics the simple sky works out by the game's rule
//! (`trp_sim::ambient::sky_simple`) against the shaders' `_SHAr` to `_SHC`
//! and `_EFT_Ambient` as the game had them. Skipped where the recording or
//! the install is not there.

use trp_data::level_light::LevelLight;
use trp_sim::ambient::sky_simple;

fn recorded() -> Option<serde_json::Value> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../local/traces/courses3/0-light-factory/light-factory.scene.json");
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn floats(v: &serde_json::Value) -> Vec<f32> {
    v.as_array().expect("an array").iter().map(|x| x.as_f64().expect("a number") as f32).collect()
}

/// A shader global as the recorder writes it: `[float, vector, texture]`.
fn global(scene: &serde_json::Value, name: &str) -> Vec<f32> {
    floats(&scene["globals"][name][1])
}

fn close(what: &str, ours: &[f32], game: &[f32], within: f32) {
    assert_eq!(ours.len(), game.len(), "{what}");
    for (a, b) in ours.iter().zip(game) {
        assert!((a - b).abs() <= within, "{what}: ours {ours:?}, the game's {game:?}");
    }
}

#[test]
fn factory_by_day_is_lit_as_the_game_lit_it() {
    let Some(scene) = recorded() else {
        eprintln!("skipped: no recording of Factory's light (local/traces/courses3)");
        return;
    };
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let light = LevelLight::factory_day(&install).expect("Factory by day's light");

    // The sun, as the recorder found it among the lights.
    let sun = scene["lights"].as_array().unwrap().iter().find(|l| l["name"] == "Weather_Factory/Light").expect("the recorded sun");
    close("the sun's turn", &light.sun.rotation, &floats(&sun["rot"]), 1e-6);
    close("the sun's colour", &light.sun.color, &floats(&sun["color"]), 1e-6);
    close("the sun's intensity", &[light.sun.intensity], &[sun["intensity"].as_f64().unwrap() as f32], 1e-6);
    close("the sun's shadow strength", &[light.sun.shadow_strength], &[sun["shadow_strength"].as_f64().unwrap() as f32], 1e-6);
    assert_eq!(light.sun.culling_mask as i32 as i64, sun["culling_mask"].as_i64().unwrap());
    close("the way the sun's light goes", &light.sun.forward, &global(&scene, "_GlobalLightDir")[..3], 1e-6);

    // The level settings, as applied to the engine's settings.
    let settings = &scene["level_settings"];
    close("the flat ambient", &light.settings.sky_color, &floats(&settings["SkyColor"]), 1e-6);
    close("the fog's colour", &light.settings.fog_color, &floats(&settings["fogColor"]), 1e-6);
    close("the fog's density", &[light.settings.fog_density], &[settings["fogDensity"].as_f64().unwrap() as f32], 1e-6);
    let direction_light_shadow = scene["globals"]["_DirectionLightShadow"][0].as_f64().unwrap() as f32;
    close("the direction light shadow", &[light.settings.direction_light_shadow], &[direction_light_shadow], 1e-6);
    close("the least smoke ambient", &light.settings.min_smoke_ambient, &global(&scene, "_MinAmbientColor"), 1e-6);

    // The ambient harmonics by the simple sky's rule.
    let sh = sky_simple(&light.sky, light.sun.forward);
    let packed = sh.packed();
    for (i, channel) in ["r", "g", "b"].iter().enumerate() {
        close(&format!("_SHA{channel}"), &packed.a[i], &global(&scene, &format!("_SHA{channel}")), 2e-6);
        close(&format!("_SHB{channel}"), &packed.b[i], &global(&scene, &format!("_SHB{channel}")), 2e-6);
    }
    close("_SHC", &packed.c, &global(&scene, "_SHC"), 2e-6);
    close("_EFT_Ambient", &sh.top(), &global(&scene, "_EFT_Ambient")[..3], 2e-6);
    eprintln!("Factory by day: the ambient from above {:?}, sideways {:?}, from below {:?}", sh.top(), sh.at([1.0, 0.0, 0.0]), sh.at([0.0, -1.0, 0.0]));
}

/// The level's camera prefab's PRISM beside the recorded camera's: the
/// tonemap's figures, the exposure's limits, the vignette and the switches
/// as the game ran them (the middle grey and the speed are the environment
/// manager's while it runs, not compared here).
#[test]
fn factorys_camera_is_the_recorded_camera() {
    let Some(scene) = recorded() else {
        eprintln!("skipped: no recording of Factory's light (local/traces/courses3)");
        return;
    };
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let light = LevelLight::factory_day(&install).expect("Factory by day's light");
    let file = light.file_of(light.settings.camera).expect("the camera's file");
    let ours = trp_data::prism::PrismSettings::of_camera(&install, &file, light.settings.camera.1).expect("the camera's PRISM");
    let game = scene["camera"]["components"].as_array().unwrap().iter().find(|c| c["type"] == "PrismEffects").expect("the recorded PRISM");
    let f = &game["fields"];
    let number = |name: &str| f[name].as_f64().unwrap_or_else(|| panic!("{name}")) as f32;
    let flag = |name: &str| f[name].as_bool().unwrap_or_else(|| panic!("{name}"));
    close("the tone", &ours.tone, &floats(&f["toneValues"]), 1e-6);
    close("the secondary tone", &ours.secondary_tone, &floats(&f["secondaryToneValues"]), 1e-6);
    close("the limits", &[ours.lower_limit, ours.upper_limit], &[number("exposureLowerLimit"), number("exposureUpperLimit")], 1e-6);
    close("the vignette", &[ours.vignette_start, ours.vignette_end, ours.vignette_strength], &[number("vignetteStart"), number("vignetteEnd"), number("vignetteStrength")], 1e-6);
    close("the vignette's colour", &ours.vignette_color, &floats(&f["vignetteColor"]), 1e-6);
    assert_eq!(f["tonemapType"], "Filmic");
    assert_eq!(ours.tonemap_type, 0);
    for (name, on) in [
        ("useTonemap", ours.use_tonemap),
        ("useExposure", ours.use_exposure),
        ("useVignette", ours.use_vignette),
        ("useBloom", ours.use_bloom),
        ("useLut", ours.use_lut),
        ("useSharpen", ours.use_sharpen),
        ("useGammaCorrection", ours.use_gamma_correction),
        ("useFog", ours.use_fog),
        ("useAmbientObscurance", ours.use_ambient_obscurance),
    ] {
        assert_eq!(on, flag(name), "{name}");
    }
    assert_eq!(ours.hist, [f["histWidth"].as_i64().unwrap() as i32, f["histHeight"].as_i64().unwrap() as i32]);
}
