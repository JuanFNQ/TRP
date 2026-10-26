//! Customs' clouds and moon as the install sets them up, beside recorder
//! 0.11.0's scene of Customs in the morning (`local/traces/courses3/
//! 0-light-outside`): the cloud controller's ambient gradient, the wind
//! controller's multipliers, the sun's cookie (its name and size, its
//! light's place), the moon's way; and the figures the clouds' passes take
//! at the recorded moment, printed. The passes' pictures were not recorded.
//! Skipped where the recording or the install is not there.

use trp_data::clouds::{LevelClouds, LevelMoon};
use trp_data::tod::TimeOfDayLevel;
use trp_data::Install;
use trp_sim::clouds::{CloudFigures, CloudInput, CloudsQuality};
use trp_sim::tod::CUSTOMS_RECORDED_MORNING;

fn recorded() -> Option<serde_json::Value> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../local/traces/courses3/0-light-outside/light-outside.scene.json");
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn f(v: &serde_json::Value) -> f32 {
    v.as_f64().unwrap_or_else(|| panic!("{v} is no number")) as f32
}

fn close(what: &str, ours: &[f32], game: &[f32], within: f32) {
    assert_eq!(ours.len(), game.len(), "{what}");
    let worst = ours.iter().zip(game).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
    eprintln!("{what}: ours {ours:?}, the game's {game:?}, worst {worst:.2e}");
    assert!(worst <= within, "{what}: ours {ours:?}, the game's {game:?}");
}

fn setup() -> Option<(serde_json::Value, Install, TimeOfDayLevel)> {
    let Some(scene) = recorded() else {
        eprintln!("skipped: no recording of Customs' light (local/traces/courses3)");
        return None;
    };
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return None;
    };
    let level = TimeOfDayLevel::customs(&install).expect("Customs' time of day");
    Some((scene, install, level))
}

#[test]
fn customs_clouds_are_read_as_the_game_has_them() {
    let Some((scene, install, level)) = setup() else { return };
    let clouds = LevelClouds::read(&install, &level).expect("Customs' clouds");
    let weather = &scene["weather"];
    // The cloud controller's ambient gradient, key by key.
    let game = &weather["CloudController"]["_ambientColor"];
    let keys = game["colors"].as_array().unwrap();
    assert_eq!(clouds.ambient_color.colors.len(), keys.len());
    for ((t, c), k) in clouds.ambient_color.colors.iter().zip(keys) {
        close("an ambient key", &[*t, c[0], c[1], c[2]], &[f(&k[0]), f(&k[1][0]), f(&k[1][1]), f(&k[1][2])], 1e-6);
    }
    assert_eq!(weather["CloudController"]["_settings"]["object"], "CloudLayerSettings_main");
    // The wind controller's multipliers and first wind.
    let wind = level.wind_controller.as_ref().expect("the weather's wind controller");
    let g = &weather["WindController"];
    close(
        "the wind's multipliers",
        &[wind.f32("RainWindMultiplier").unwrap(), wind.f32("CloudWindMultiplier").unwrap(), wind.f32("BilboardTreesWindMultiplier").unwrap()],
        &[f(&g["RainWindMultiplier"]), f(&g["CloudWindMultiplier"]), f(&g["BilboardTreesWindMultiplier"])],
        0.0,
    );
    // The sun's light carries the cookie the clouds' shadows are baked
    // into, as wide as the settings say.
    let sun = scene["lights"].as_array().unwrap().iter().find(|l| l["name"] == "Weather/Sky Dome/Light").expect("the recorded sun");
    assert_eq!(sun["cookie"], "Cloud Shadows");
    close("the cookie's size", &[clouds.settings.shadow_size], &[f(&sun["cookie_size"])], 0.0);
    let place: Vec<f32> = sun["pos"].as_array().unwrap().iter().map(f).collect();
    close("the sun's light's place", &level.sun_position, &place, 1e-3);
    // The recorded wind is what the wind controller handed the shaders.
    let shaders: Vec<f32> = scene["globals"]["_WindVector"][1].as_array().unwrap().iter().map(f).collect();
    close("the wind", &CUSTOMS_RECORDED_MORNING.wind, &shaders[..2], 1e-7);
    eprintln!("the remap at the recorded cloudiness: {:?}", clouds.remap.at((CUSTOMS_RECORDED_MORNING.cloudiness + 1.0) / 2.0));
    eprintln!("the map: {} {}x{}, clamped {:?}", clouds.map.name, clouds.map.width, clouds.map.height, clouds.map.clamp);
}

/// The figures of the passes at the recorded moment, from the morning as
/// worked out (the light's turn, colour and intensity, the harmonics, the
/// way to the sun are compared with the recording in `time_of_day.rs`).
#[test]
fn customs_morning_clouds_figures() {
    let Some((scene, install, level)) = setup() else { return };
    let clouds = LevelClouds::read(&install, &level).expect("Customs' clouds");
    let sky = trp_sim::tod::Sky::of_level(&level).unwrap();
    let stored = trp_sim::tod::stored_atmosphere(&level.sky).unwrap();
    let m = CUSTOMS_RECORDED_MORNING;
    let state = sky.settle(m.hour, m.cloudiness, stored, 12);
    let input = CloudInput {
        cloudiness: m.cloudiness,
        wind: m.wind,
        cloud_wind_multiplier: level.wind_controller.as_ref().unwrap().f32("CloudWindMultiplier").unwrap(),
        light_rotation: state.light_rotation,
        light_color: state.light_color,
        light_intensity: state.light_intensity,
        harmonics: state.harmonics,
        sun_direction: state.sun_direction,
    };
    let figures = CloudFigures::of(&clouds, CloudsQuality::Low, &input);
    eprintln!("{figures:#?}");
    // The wind's 0.378 of its top, times two, halved: about 50 km/h.
    assert!((figures.scroll_speed - (20.0 + 80.0 * 0.378_197)).abs() < 1e-3, "{}", figures.scroll_speed);
    // The cookie's axes are the recorded light's.
    let sun = scene["lights"].as_array().unwrap().iter().find(|l| l["name"] == "Weather/Sky Dome/Light").unwrap();
    let turn: Vec<f32> = sun["rot"].as_array().unwrap().iter().map(f).collect();
    let rotate = |v: [f32; 3]| trp_data::level_light::rotate([turn[0], turn[1], turn[2], turn[3]], v);
    let shadows = figures.shadows.expect("the clouds cast shadows at low");
    close("the light's right", &shadows.sun_right, &rotate([1.0, 0.0, 0.0]), 2e-4);
    close("the light's up", &shadows.sun_up, &rotate([0.0, 1.0, 0.0]), 2e-4);
    // The moon's way, the dome's own on Customs (unturned).
    let moon: Vec<f32> = scene["sky"]["now"]["MoonDirection"].as_array().expect("the recorded moon's way").iter().map(f).collect();
    close("the moon's way", &state.moon_direction, &moon, 1e-4);
    let globals = sky.globals(&state);
    eprintln!("the moon: {:?}", globals.moon_mesh);
    let gamma = |c: f32| (1.055 * c.max(0.0).powf(0.416_667) - 0.055).max(0.0);
    for (name, way) in [("down", [0.0, -1.0, 0.0]), ("up", [0.0, 1.0, 0.0]), ("side", [1.0, 0.0, 0.0])] {
        eprintln!("the highlight's harmonics {name}: {:?}, the sky's {:?}", state.harmonics_without_top.at(way).map(gamma), state.harmonics.at(way).map(gamma));
    }
    let read = LevelMoon::read(&install, &level).expect("the moon");
    eprintln!("the moon's sphere {} ({} vertices), its picture {} {}x{}", read.mesh.name, read.mesh.positions.len(), read.picture.name, read.picture.width, read.picture.height);
}
