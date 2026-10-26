//! Customs' time of day as the install sets it up, beside recorder
//! 0.11.0's scene of Customs in the morning (course 0's second step, kept in
//! `local/traces/courses3/0-light-outside`): every field of the sky
//! (`TOD_Sky`) and of the weather's time-of-day controller read from the
//! level's scene against the recorder's copy of the same scripts (the
//! fields the game changes while it runs are left out and named). Skipped
//! where the recording or the install is not there.

use trp_data::particles::Gradient;
use trp_data::tod::{FieldValue, Fields, TimeOfDayLevel, WeightedCurve};

fn recorded() -> Option<serde_json::Value> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../local/traces/courses3/0-light-outside/light-outside.scene.json");
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn f(v: &serde_json::Value) -> f32 {
    v.as_f64().unwrap_or_else(|| panic!("{v} is no number")) as f32
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= 1e-5 * (1.0 + a.abs().max(b.abs()))
}

fn gradient_matches(ours: &Gradient, game: &serde_json::Value) -> bool {
    let colors = game["colors"].as_array().unwrap();
    let alphas = game["alphas"].as_array().unwrap();
    ours.colors.len() == colors.len()
        && ours.alphas.len() == alphas.len()
        && ours.colors.iter().zip(colors).all(|((t, c), g)| near(*t, f(&g[0])) && (0..3).all(|i| near(c[i], f(&g[1][i]))))
        && ours.alphas.iter().zip(alphas).all(|((t, a), g)| near(*t, f(&g[0])) && near(*a, f(&g[1])))
}

/// The recorder writes a key as time, value, the slopes in and out, the
/// weights in and out and the weighted mode.
fn curve_matches(ours: &WeightedCurve, game: &serde_json::Value) -> bool {
    let keys = game["keys"].as_array().unwrap();
    ours.keys.len() == keys.len()
        && ours.keys.iter().zip(keys).all(|(k, g)| {
            near(k.time, f(&g[0])) && near(k.value, f(&g[1])) && near(k.in_slope, f(&g[2])) && near(k.out_slope, f(&g[3])) && near(k.in_weight, f(&g[4])) && near(k.out_weight, f(&g[5])) && k.mode as i64 == g[6].as_i64().unwrap_or(-1)
        })
}

/// Every field of ours whose recorded twin is there: compared; the count of
/// each, and the names that differ.
fn compare(ours: &Fields, game: &serde_json::Value, skipped: &[&str]) -> (usize, Vec<String>) {
    let mut checked = 0;
    let mut differ = Vec::new();
    for (name, value) in &ours.0 {
        if skipped.iter().any(|s| name.starts_with(s)) {
            continue;
        }
        // The recorder writes an auto-property's backing field by its own name.
        let mut at = game;
        for part in name.split('.') {
            let backing = format!("<{part}>k__BackingField");
            at = if at.get(part).is_some() { &at[part] } else { &at[backing.as_str()] };
        }
        if at.is_null() {
            continue;
        }
        let same = match value {
            FieldValue::Number(v) => near(*v, f(at)),
            FieldValue::Int(v) => at.as_i64().map_or(true, |g| g as i32 == *v),
            FieldValue::Flag(v) => at.as_bool() == Some(*v),
            FieldValue::Color(c) => (0..4).all(|i| near(c[i], f(&at[i]))),
            FieldValue::Vec2(c) => (0..2).all(|i| near(c[i], f(&at[i]))),
            FieldValue::Vec3(c) => (0..3).all(|i| near(c[i], f(&at[i]))),
            FieldValue::Gradient(g) => gradient_matches(g, at),
            FieldValue::Curve(c) => curve_matches(c, at),
            FieldValue::Pointer(_) => true,
        };
        checked += 1;
        if !same {
            differ.push(format!("{name}: ours {value:?}, the game's {at}"));
        }
    }
    (checked, differ)
}

#[test]
fn customs_time_of_day_is_read_as_the_game_has_it() {
    let Some(scene) = recorded() else {
        eprintln!("skipped: no recording of Customs' light (local/traces/courses3)");
        return;
    };
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let level = TimeOfDayLevel::customs(&install).expect("Customs' time of day");
    // The game sets these while it runs: the atmosphere (its controller,
    // each frame), the cycle (the raid's time), the night's colour (by the
    // moon's angle).
    let (n, differ) = compare(&level.sky, &scene["sky"]["fields"], &["Atmosphere", "Cycle", "Night.ColorMultiplier"]);
    eprintln!("the sky: {n} fields compared");
    assert!(differ.is_empty(), "the sky's fields differ:\n{}", differ.join("\n"));
    // The weather controller sets the contrast's top by the cloudiness on
    // every frame (`WeatherController.method_4`: 1.5 less half of it, held
    // between -1 and 0.4), which is how the recorded moment's cloudiness is
    // known: the stored 1.5 and the recorded top give it.
    let (n, differ) = compare(&level.weather, &scene["weather"], &["TimeOfDayController.ContrastMinMax"]);
    eprintln!("the weather: {n} fields compared");
    assert!(differ.is_empty(), "the weather's fields differ:\n{}", differ.join("\n"));
    assert!(n >= 39, "too few of the weather's fields found in the recording ({n})");
    let stored = level.weather.vec2("TimeOfDayController.ContrastMinMax").unwrap();
    let ran = f(&scene["weather"]["TimeOfDayController"]["ContrastMinMax"][1]);
    let cloudiness = (1.5 - ran) / 0.5;
    assert_eq!(stored[1], 1.5);
    eprintln!("the recorded moment's cloudiness: {cloudiness}");
    assert!((cloudiness - RECORDED_CLOUDINESS).abs() < 1e-5);
}

/// The weather's cloudiness when Customs' morning was recorded, as the
/// contrast's top shows it (above): the app's.
const RECORDED_CLOUDINESS: f32 = trp_sim::tod::CUSTOMS_RECORDED_MORNING.cloudiness;

/// Customs' morning worked out by the game's rules from the install's
/// figures, the recorded hour and the recorded moment's cloudiness, beside
/// what the game had: the atmosphere, the sun's way, the light's turn,
/// colour and intensity, the night's colour, the five samples' sum and every
/// harmonic of the ambient.
#[test]
fn customs_morning_is_worked_out_as_the_game_had_it() {
    let Some(scene) = recorded() else {
        eprintln!("skipped: no recording of Customs' light (local/traces/courses3)");
        return;
    };
    let Ok(install) = crate::fixture::install().ok_or(()) else {
        return;
    };
    let level = TimeOfDayLevel::customs(&install).expect("Customs' time of day");
    let sky = trp_sim::tod::Sky::of_level(&level).expect("the sky's figures");
    let hour = f(&scene["sky"]["fields"]["<Cycle>k__BackingField"]["Hour"]);
    assert_eq!(hour, trp_sim::tod::CUSTOMS_RECORDED_MORNING.hour);
    let stored = trp_sim::tod::stored_atmosphere(&level.sky).unwrap();
    let state = sky.settle(hour, RECORDED_CLOUDINESS, stored, 12);
    let close = |what: &str, ours: &[f32], game: &[f32], within: f32| {
        let worst = ours.iter().zip(game).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
        eprintln!("{what}: ours {ours:?}, the game's {game:?}, worst {worst:.2e}");
        assert!(worst <= within, "{what}: ours {ours:?}, the game's {game:?}");
    };
    let g = &scene["sky"]["fields"]["<Atmosphere>k__BackingField"];
    let a = &state.atmosphere;
    close(
        "the atmosphere",
        &[a.rayleigh_multiplier, a.mie_multiplier, a.brightness, a.scattering_brightness, a.contrast, a.directionality, a.fogginess],
        &[f(&g["RayleighMultiplier"]), f(&g["MieMultiplier"]), f(&g["Brightness"]), f(&g["ScatteringBrightness"]), f(&g["Contrast"]), f(&g["Directionality"]), f(&g["Fogginess"])],
        1e-4,
    );
    let global = |name: &str| -> Vec<f32> { scene["globals"][name][1].as_array().unwrap().iter().map(f).collect() };
    close("toward the sun", &state.sun_direction, &global("TOD_LightDirection")[..3], 1e-4);
    let sun = scene["lights"].as_array().unwrap().iter().find(|l| l["name"] == "Weather/Sky Dome/Light").expect("the recorded sun");
    let turn: Vec<f32> = sun["rot"].as_array().unwrap().iter().map(f).collect();
    // A turn and its negative are the same turn.
    let sign = if state.light_rotation.iter().zip(&turn).map(|(a, b)| a * b).sum::<f32>() < 0.0 { -1.0 } else { 1.0 };
    close("the light's turn", &state.light_rotation.map(|v| v * sign), &turn, 1e-4);
    let colour: Vec<f32> = sun["color"].as_array().unwrap().iter().map(f).collect();
    close("the light's colour", &state.light_color, &colour[..3], 1e-4);
    close("the light's intensity", &[state.light_intensity], &[f(&sun["intensity"])], 1e-5);
    close("the night's colour multiplier", &[state.night_color_multiplier], &[f(&scene["sky"]["fields"]["Night"]["ColorMultiplier"])], 1e-4);
    let top: Vec<f32> = scene["weather"]["TimeOfDayController"]["<TopHorizontSkyColor>k__BackingField"].as_array().unwrap().iter().map(f).collect();
    close("the five samples' sum", &state.top_horizon_sky_color, &top[..3], 1e-4);
    let packed = state.harmonics.packed();
    for (i, channel) in ["r", "g", "b"].iter().enumerate() {
        close(&format!("_SHA{channel}"), &packed.a[i], &global(&format!("_SHA{channel}")), 2e-5);
        close(&format!("_SHB{channel}"), &packed.b[i], &global(&format!("_SHB{channel}")), 2e-5);
    }
    close("_SHC", &packed.c, &global("_SHC"), 2e-5);
    close("_EFT_Ambient", &state.harmonics.top(), &global("_EFT_Ambient")[..3], 2e-5);
}

/// Customs' camera (its level settings' camera prefab) is the recorded
/// camera's PRISM; the level's flat ambient and the engine's harmonics are
/// nought, as recorded (the time of day's harmonics are the shaders' own).
#[test]
fn customs_camera_and_flat_ambient_are_the_recorded() {
    let Some(scene) = recorded() else { return };
    let Ok(install) = crate::fixture::install().ok_or(()) else { return };
    let level = TimeOfDayLevel::customs(&install).expect("Customs' time of day");
    let file = level.file_of(level.settings.camera).expect("the camera's file");
    let ours = trp_data::prism::PrismSettings::of_camera(&install, &file, level.settings.camera.1).expect("the camera's PRISM");
    let game = scene["camera"]["components"].as_array().unwrap().iter().find(|c| c["type"] == "PrismEffects").expect("the recorded PRISM");
    let g = &game["fields"];
    let numbers = |v: &serde_json::Value| -> Vec<f32> { v.as_array().unwrap().iter().map(f).collect() };
    let same = |what: &str, ours: &[f32], game: &[f32]| assert!(ours.len() == game.len() && ours.iter().zip(game).all(|(a, b)| near(*a, *b)), "{what}: ours {ours:?}, the game's {game:?}");
    same("the tone", &ours.tone, &numbers(&g["toneValues"]));
    same("the limits", &[ours.lower_limit, ours.upper_limit], &[f(&g["exposureLowerLimit"]), f(&g["exposureUpperLimit"])]);
    same("the vignette", &[ours.vignette_start, ours.vignette_end, ours.vignette_strength], &[f(&g["vignetteStart"]), f(&g["vignetteEnd"]), f(&g["vignetteStrength"])]);
    assert_eq!(g["tonemapType"], "Filmic");
    for (name, on) in [("useTonemap", ours.use_tonemap), ("useExposure", ours.use_exposure), ("useVignette", ours.use_vignette), ("useBloom", ours.use_bloom), ("useLut", ours.use_lut)] {
        assert_eq!(Some(on), g[name].as_bool(), "{name}");
    }
    // The camera's own colour filter, on as shipped and as it ran.
    let filter = trp_data::prism::ColorFilter::of_camera(&install, &file, level.settings.camera.1).expect("the camera's filter").expect("a CC_Vintage");
    let vintage = scene["camera"]["components"].as_array().unwrap().iter().find(|c| c["type"] == "CC_Vintage").expect("the recorded CC_Vintage");
    assert_eq!(Some(filter.enabled), vintage["enabled"].as_bool());
    assert_eq!(Some(filter.name().unwrap()), vintage["fields"]["filter"].as_str());
    same("the filter's amount", &[filter.amount], &[f(&vintage["fields"]["amount"])]);
    let table = filter.table(&install).expect("the filter's table");
    assert_eq!((table.width, table.height, table.rgba.len()), (512, 512, 512 * 512 * 4));
    eprintln!("the filter's table: clamped {:?}, point {}", table.clamp, table.point);
    // The camera's scattering over the scene, as shipped and as it ran.
    let scattering = trp_data::prism::CameraScattering::of_camera(&install, &file, level.settings.camera.1).expect("the camera's scattering").expect("a TOD_Scattering");
    let ran = scene["camera"]["components"].as_array().unwrap().iter().find(|c| c["type"] == "TOD_Scattering").expect("the recorded TOD_Scattering");
    let g = &ran["fields"];
    assert_eq!(Some(scattering.enabled), ran["enabled"].as_bool());
    assert_eq!((Some(scattering.lighten), Some(scattering.from_level_settings)), (g["Lighten"].as_bool(), g["FromLevelSettings"].as_bool()));
    // Shipped without MBOIT; the weather sets it as the camera is made
    // (`WeatherController.OnCameraChanged`), which hands the fog to
    // `MBOIT_Scattering` only while the windows' manager is active: in the
    // raid recorded, not, and this one ran.
    assert!(!scattering.mboit && g["_mboit"].as_bool() == Some(true));
    let other = scene["camera"]["components"].as_array().unwrap().iter().find(|c| c["type"] == "MBOIT_Scattering").expect("the recorded MBOIT_Scattering");
    assert_eq!(other["enabled"].as_bool(), Some(false));
    same("the scattering's density and glow", &[scattering.global_density, scattering.sunrise_glow], &[f(&g["GlobalDensity"]), f(&g["SunrizeGlow"])]);
    // Its height falloff and zero level the level settings' as it ran.
    same("the scattering's height", &[level.settings.height_falloff, level.settings.zero_level], &[f(&g["HeightFalloff"]), f(&g["ZeroLevel"])]);
    let (dither, w, h) = scattering.dithering(&install).expect("the dithering");
    eprintln!("the dithering {w}x{h}: {dither:?}; the prefab's own height falloff {} and zero level {}", scattering.height_falloff, scattering.zero_level);
    let r = &scene["render_settings"];
    assert_eq!(r["ambient_mode"], "Flat");
    same("the flat ambient", &level.settings.sky_color, &numbers(&r["ambient_light"]));
    assert!(r["ambient_probe"].as_array().unwrap().iter().flat_map(|c| c.as_array().unwrap()).all(|v| f(v) == 0.0));
}

/// Customs' indoor triggers as the install has them: their common figures
/// (the hall's: `range` and `prism_view`), and the stencil volume of its
/// concrete warehouse (`sklad_concrete`, in `custom_factoryStorageZone`):
/// the share of the sky's ambient it takes away, whole, and no fog's.
#[test]
fn customs_indoor_figures_are_the_installs() {
    let Ok(install) = crate::fixture::install().ok_or(()) else { return };
    let level = TimeOfDayLevel::customs(&install).expect("Customs' time of day");
    assert_eq!(level.indoor_triggers.len(), 79);
    let (common, many) = level.common_indoor().expect("a trigger");
    eprintln!("the common indoor figures {common:?}, of {many}");
    assert_eq!((common.fade_time, common.exposure_speed, common.exposure_offset), (1.0, 4.0, 0.14));
    assert!(many >= 60);
    let index = trp_data::screen::scene_paths(&install).unwrap().iter().position(|p| p.ends_with("custom_factoryStorageZone.unity")).unwrap();
    let scene = trp_data::screen::Scene::load(&install, &format!("level{index}"), true).unwrap();
    let warehouse = scene.behaviours.iter().find(|b| scene.path(b.node).ends_with("sklad_concrete/stencil") && scene.behaviour(b.node, "StencilShadow").is_some_and(|s| s.id == b.id)).expect("the warehouse's stencil");
    let fields = scene.fields(warehouse);
    let f = |at: usize| f32::from_le_bytes(fields[at..at + 4].try_into().unwrap());
    // `Ambient` (its alpha is the share), the culling (8 bytes), the draw
    // priority, `FogAttenuation`.
    assert_eq!((f(12), f(28)), (1.0, 0.0));
}

/// What the sky's draw needs at the recorded moment, printed.
#[test]
fn customs_morning_sky_figures() {
    let Some(scene) = recorded() else { return };
    let Ok(install) = crate::fixture::install().ok_or(()) else { return };
    let level = TimeOfDayLevel::customs(&install).expect("Customs' time of day");
    let sky = trp_sim::tod::Sky::of_level(&level).expect("the sky's figures");
    let hour = f(&scene["sky"]["fields"]["<Cycle>k__BackingField"]["Hour"]);
    let state = sky.settle(hour, RECORDED_CLOUDINESS, trp_sim::tod::stored_atmosphere(&level.sky).unwrap(), 12);
    eprintln!("dome turn {:?} at {:?}, north {}", level.dome_rotation, level.dome_position, level.settings.north);
    eprintln!("sun zenith {} moon zenith {} lerp {}", state.sun_zenith, state.moon_zenith, state.lerp_value);
    eprintln!("sky {:?} moon sky {:?} ambient {:?} halo {:?}", state.sun_sky_color, state.moon_sky_color, state.ambient_color, state.moon_halo_color);
    eprintln!("settings {:?}", level.settings);
    eprintln!("outdoor {:?}", level.outdoor);
    eprintln!("sun {:?}", level.sun);
    eprintln!("recorded render settings {}", scene["render_settings"]);
}

/// The atmosphere's mesh as the install has it: an icosphere of radius one
/// (four times divided: 2,562 vertices, 5,120 triangles).
#[test]
fn customs_atmosphere_is_an_icosphere() {
    let Ok(install) = crate::fixture::install().ok_or(()) else { return };
    let level = TimeOfDayLevel::customs(&install).expect("Customs' time of day");
    let mesh = &level.atmosphere_mesh;
    assert_eq!(mesh.name, "Icosphere");
    assert_eq!((mesh.positions.len(), mesh.triangles.len()), (2562, 15360));
    for p in &mesh.positions {
        let r = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        assert!((r - 1.0).abs() < 1e-3, "a vertex at {r}");
    }
    assert!(mesh.triangles.iter().all(|&i| (i as usize) < mesh.positions.len()));
}
