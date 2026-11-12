//! The options' file, saving and loading, the kept name.

use super::*;

/// The app's own file for what is changed here: `options.json` in the
/// workspace's `local` folder, where the first start writes the install's
/// path (`setup::local_folder`).
pub(crate) fn options_file() -> PathBuf {
    crate::platform::setup::local_folder().join("options.json")
}

fn source_json(source: &Source) -> serde_json::Value {
    let keys: Vec<String> = source.keys.iter().map(|key| key_name(*key)).collect();
    match source.wheel {
        Some(up) => serde_json::json!({ "keys": keys, "wheel": up }),
        None => serde_json::json!({ "keys": keys }),
    }
}

fn source_of_json(value: &serde_json::Value) -> Source {
    let names = value.get("keys").and_then(|keys| keys.as_array());
    let keys = names.map(|names| names.iter().filter_map(|name| code(name.as_str()?)).collect()).unwrap_or_default();
    Source { keys, wheel: value.get("wheel").and_then(|up| up.as_bool()) }
}

/// The skills as the app's file keeps them: each skill not at 0 by the
/// game's name, with its progress (a hundred a level).
fn skills_json(skills: &trp_sim::condition::skills::SkillLevels) -> serde_json::Value {
    let kept = trp_sim::condition::skills::Skill::ALL.iter().filter(|skill| skills.progress(**skill) > 0.0).map(|skill| (skill.name().to_owned(), serde_json::json!(skills.progress(*skill))));
    serde_json::Value::Object(kept.collect())
}

/// The weapon mastering as the app's file keeps it: each begun family's
/// name and points, in the order begun (the game's profile keeps the same,
/// `SkillsDescriptor.Mastering`), the MASTERING page's order.
fn mastering_json(mastering: &trp_sim::condition::mastering::MasteringLevels) -> serde_json::Value {
    serde_json::Value::Array(mastering.begun().iter().map(|(name, points)| serde_json::json!([name, points])).collect())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn save(path: &std::path::Path, controls: &Controls, graphics: &Graphics, mouse: &Mouse, switches: &Switches, (game, notices, hide_tabs, experience): (&crate::panels::battle_view::BattleModes, &Notices, bool, i64), sound: &crate::audio::ui_sound::Levels, skills: &trp_sim::condition::skills::SkillLevels, mastering: &trp_sim::condition::mastering::MasteringLevels) -> std::io::Result<()> {
    let keys: Vec<serde_json::Value> = controls
        .keys
        .iter()
        .map(|group| serde_json::json!({ "name": group.name, "press": group.press.name(), "variants": group.variants.iter().map(source_json).collect::<Vec<_>>() }))
        .collect();
    let axes: Vec<serde_json::Value> = controls
        .axes
        .iter()
        .map(|axis| serde_json::json!({ "name": axis.name, "pairs": axis.pairs.iter().map(|pair| pair.iter().map(source_json).collect::<Vec<_>>()).collect::<Vec<_>>() }))
        .collect();
    let mode = match graphics.mode {
        Mode::Fullscreen => "fullscreen",
        Mode::Borderless => "borderless",
        Mode::Windowed => "windowed",
    };
    let all = serde_json::json!({
        "controls": { "double_click": controls.double_click, "keys": keys, "axes": axes },
        "mouse": { "sensitivity": mouse.sensitivity, "aiming_sensitivity": mouse.aiming, "optic_sensitivity": mouse.optic, "inverted_x": mouse.invert[0], "inverted_y": mouse.invert[1] },
        "graphics": { "vsync": graphics.vsync, "field_of_view": graphics.field_of_view, "mode": mode, "resolution": graphics.resolution, "texture_quality": graphics.texture_quality, "mip_streaming": graphics.mip_streaming, "shadow_quality": graphics.shadow_quality, "ssao": graphics.ssao.key(), "clouds_quality": graphics.clouds_quality.key(), "render_thread": graphics.render_thread, "game_fps": graphics.game_fps, "fps_limit_off": graphics.fps_limit_off, "msaa": graphics.msaa, "dust_resolution": graphics.dust_resolution.key(), "renderer": graphics.renderer.key(), "lod_bias": graphics.lod_bias },
        "switches": Switch::ALL.iter().map(|switch| (switch.key().to_owned(), serde_json::Value::Bool(switches.on(*switch)))).collect::<serde_json::Map<_, _>>(),
        "game": { "quick_slots": game.quick.name(), "stamina": game.stamina.name(), "health": game.health.name(), "health_color": game.color_name(), "tactical_input": game.tactical.word() },
        "notices": Notices::ALL.iter().map(|(which, ..)| (Notices::KEYS[*which].to_owned(), serde_json::Value::Bool(notices.on(*which)))).collect::<serde_json::Map<_, _>>(),
        "hide_unused_tabs": hide_tabs,
        "sound": { "overall": sound.overall, "interface": sound.interface },
        "skills": skills_json(skills),
        "mastering": mastering_json(mastering),
        "experience": experience,
    });
    // The suits chosen on the OVERALL tab are kept in the same file by
    // their own module (`clothing`), and kept as they are.
    let mut all = all;
    // So are the kinds examined (`examine_view`).
    let old = crate::platform::options_store::read(path);
    for key in [crate::player::clothing::KEPT_AS, crate::screens::examine_view::KEPT_AS, crate::player::gestures::KEPT_AS, crate::audio::voice_sound::KEPT_AS, NAME_KEPT_AS] {
        if let Some(kept) = old.as_ref().and_then(|old| old.get(key).cloned()) {
            all[key] = kept;
        }
    }
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(&all).unwrap_or_default())
}

/// Where the character's name is kept in the options' file, by itself and
/// at once as it is typed (as the gestures' binds are).
pub const NAME_KEPT_AS: &str = "character_name";

/// The most letters the CHARACTER NAME field holds. `ESTIMATE`: the game's
/// own field caps a nickname at its `characterLimit`
/// (`ValidationInputField`), a figure of the registration screen not read
/// from the install; 15 is the app's.
pub(super) const NAME_MOST: usize = 15;

/// The character's name as the options' file keeps it, if it does (an
/// empty one kept as such).
pub(super) fn kept_name(file: &std::path::Path) -> Option<String> {
    let all = crate::platform::options_store::read(file)?;
    all.get(NAME_KEPT_AS)?.as_str().map(|name| name.chars().take(NAME_MOST).collect())
}

/// Keeps the character's name in the options' file, all else in it as it
/// was.
pub(super) fn keep_name(file: &std::path::Path, name: &str) {
    crate::platform::options_store::keep(file, NAME_KEPT_AS, Some(serde_json::Value::String(name.to_owned())), "the character's name");
}

/// Reads the app's file over the install's bindings and the display's
/// and the mouse's settings as the run began with them: a binding or an
/// axis the file names takes the file's keys, and the rest stay the
/// install's; a switch of the FEATURES page the
/// file has is as the file has it, where what it needs is on (the older
/// list's `"features"` is not read: the switches begin as
/// recommended). Says which of the first two parts the file had.
#[allow(clippy::too_many_arguments)]
pub(super) fn load(path: &std::path::Path, controls: &mut Controls, graphics: &mut Graphics, mouse: &mut Mouse, switches: &mut Switches, (game, notices, hide_tabs, experience): (&mut crate::panels::battle_view::BattleModes, &mut Notices, &mut bool, &mut i64), sound: &mut crate::audio::ui_sound::Levels, skills: &mut trp_sim::condition::skills::SkillLevels, mastering: &mut trp_sim::condition::mastering::MasteringLevels) -> [bool; 2] {
    let Some(all) = crate::platform::options_store::read(path) else { return [false; 2] };
    if let Some(kept) = all.get("experience").and_then(|kept| kept.as_i64()) {
        *experience = kept.max(0);
    }
    // (Once kept as `"xp_notices"`.)
    let kept_notices = all.get("notices").or_else(|| all.get("xp_notices"));
    for (which, ..) in Notices::ALL {
        if let Some(on) = kept_notices.and_then(|kept| kept.get(Notices::KEYS[which])?.as_bool()) {
            notices.set(which, on);
        }
    }
    if let Some(hidden) = all.get("hide_unused_tabs").and_then(|kept| kept.as_bool()) {
        *hide_tabs = hidden;
    }
    for pair in all.get("mastering").and_then(|kept| kept.as_array()).into_iter().flatten() {
        match (pair.get(0).and_then(|name| name.as_str()), pair.get(1).and_then(|points| points.as_f64())) {
            (Some(name), Some(points)) => mastering.set_points(name, points as f32),
            _ => eprintln!("options: a weapon family's mastering not read: {pair}"),
        }
    }
    if let Some(kept) = all.get("skills").and_then(|kept| kept.as_object()) {
        for (name, progress) in kept {
            match (trp_sim::condition::skills::Skill::of_name(name), progress.as_f64()) {
                (Some(skill), Some(progress)) => skills.set_progress(skill, progress as f32),
                _ => eprintln!("options: no skill {name} to keep"),
            }
        }
    }
    if let Some(kept) = all.get("sound") {
        let level = |name: &str, now: u8| kept.get(name).and_then(|v| v.as_u64()).map_or(now, |v| v.min(10) as u8);
        (sound.overall, sound.interface) = (level("overall", sound.overall), level("interface", sound.interface));
    }
    if let Some(kept) = all.get("game") {
        use crate::panels::battle_view::Shows;
        let shows = |name: &str, now: Shows| kept.get(name).and_then(|v| Shows::of(v.as_str()?)).unwrap_or(now);
        game.quick = shows("quick_slots", game.quick);
        game.stamina = shows("stamina", game.stamina);
        game.health = shows("health", game.health);
        if let Some(scheme) = kept.get("health_color").and_then(|v| v.as_str()) {
            game.green_to_red = scheme == "GreenToRed";
        }
        if let Some(mode) = kept.get("tactical_input").and_then(|v| v.as_str()).and_then(trp_sim::weapons::tactical::InputMode::of_word) {
            game.tactical = mode;
        }
    }
    let kept_switch = |name: &str| all.get("switches").and_then(|kept| kept.get(name)?.as_bool());
    for switch in Switch::ALL {
        if let Some(on) = kept_switch(switch.key()) {
            switches.0[switch as usize] = on;
        }
    }
    switches.settle();
    if let Some(kept) = all.get("mouse") {
        let figure = |name: &str, kind: Slide, now: f32| kept.get(name).and_then(|v| v.as_f64()).map_or(now, |v| (v as f32).clamp(kind.range()[0], kind.range()[1]));
        mouse.sensitivity = figure("sensitivity", Slide::Sensitivity, mouse.sensitivity);
        mouse.aiming = figure("aiming_sensitivity", Slide::Aiming, mouse.aiming);
        mouse.optic = figure("optic_sensitivity", Slide::Optic, mouse.optic);
        for (at, name) in ["inverted_x", "inverted_y"].into_iter().enumerate() {
            mouse.invert[at] = kept.get(name).and_then(|v| v.as_bool()).unwrap_or(mouse.invert[at]);
        }
    }
    let list = |part: &str, list: &str| all.get(part).and_then(|part| part.get(list)?.as_array()).cloned().unwrap_or_default();
    let named = |entry: &serde_json::Value| entry.get("name").and_then(|name| name.as_str()).map(str::to_owned);
    for entry in list("controls", "keys") {
        let Some(group) = named(&entry).and_then(|name| controls.keys.iter_mut().find(|group| group.name == name)) else { continue };
        let variants = entry.get("variants").and_then(|variants| variants.as_array()).cloned().unwrap_or_default();
        group.variants = std::array::from_fn(|at| variants.get(at).map(source_of_json).unwrap_or_default());
        group.press = entry.get("press").and_then(|press| PressType::of(press.as_str()?)).unwrap_or(group.press);
    }
    for entry in list("controls", "axes") {
        let Some(axis) = named(&entry).and_then(|name| controls.axes.iter_mut().find(|axis| axis.name == name)) else { continue };
        let pairs = entry.get("pairs").and_then(|pairs| pairs.as_array()).cloned().unwrap_or_default();
        axis.pairs = std::array::from_fn(|pair| std::array::from_fn(|way| pairs.get(pair).and_then(|pair| pair.get(way)).map(source_of_json).unwrap_or_default()));
    }
    if let Some(seconds) = all.get("controls").and_then(|controls| controls.get("double_click")?.as_f64()) {
        controls.double_click = (seconds as f32).clamp(0.3, 1.0);
    }
    let Some(display) = all.get("graphics") else { return [all.get("controls").is_some(), false] };
    graphics.vsync = display.get("vsync").and_then(|vsync| vsync.as_bool()).unwrap_or(graphics.vsync);
    graphics.field_of_view = display.get("field_of_view").and_then(|fov| fov.as_f64()).map_or(graphics.field_of_view, |fov| (fov as f32).clamp(FOV[0], FOV[1]));
    graphics.mode = match display.get("mode").and_then(|mode| mode.as_str()) {
        Some("fullscreen") => Mode::Fullscreen,
        Some("borderless") => Mode::Borderless,
        Some("windowed") => Mode::Windowed,
        _ => graphics.mode,
    };
    graphics.texture_quality = display.get("texture_quality").and_then(|quality| quality.as_u64()).map_or(graphics.texture_quality, |quality| quality.min(2) as u8);
    graphics.mip_streaming = display.get("mip_streaming").and_then(|on| on.as_bool()).unwrap_or(graphics.mip_streaming);
    graphics.shadow_quality = display.get("shadow_quality").and_then(|quality| quality.as_u64()).map_or(graphics.shadow_quality, |quality| quality.min(3) as u8);
    graphics.ssao = display.get("ssao").and_then(|key| trp_sim::lighting::hbao::SsaoMode::from_key(key.as_str()?)).unwrap_or(graphics.ssao);
    graphics.clouds_quality = display.get("clouds_quality").and_then(|key| trp_sim::lighting::clouds::CloudsQuality::from_key(key.as_str()?)).unwrap_or(graphics.clouds_quality);
    graphics.render_thread = display.get("render_thread").and_then(|on| on.as_bool()).unwrap_or(graphics.render_thread);
    graphics.game_fps = display.get("game_fps").and_then(|fps| fps.as_u64()).map_or(graphics.game_fps, |fps| (fps.min(1000) as u32).clamp(GAME_FPS[0], GAME_FPS[1]));
    graphics.fps_limit_off = display.get("fps_limit_off").and_then(|off| off.as_bool()).unwrap_or(graphics.fps_limit_off);
    graphics.msaa = display.get("msaa").and_then(|samples| samples.as_u64()).and_then(|samples| MSAA.iter().find(|(s, _)| *s as u64 == samples).map(|(s, _)| *s)).unwrap_or(graphics.msaa);
    graphics.dust_resolution = display.get("dust_resolution").and_then(|key| crate::effects::offscreen_dust::DustResolution::from_key(key.as_str()?)).unwrap_or(graphics.dust_resolution);
    graphics.renderer = display.get("renderer").and_then(|key| crate::platform::backend::Renderer::from_key(key.as_str()?)).unwrap_or(graphics.renderer);
    graphics.lod_bias = display.get("lod_bias").and_then(|bias| bias.as_f64()).map_or(graphics.lod_bias, |bias| trp_data::setup::install::lod_bias(bias as f32));
    let side = |at: usize| display.get("resolution")?.get(at)?.as_u64().map(|side| side as u32);
    if let (Some(width), Some(height)) = (side(0), side(1)) {
        graphics.resolution = [width.max(640), height.max(360)];
    }
    [all.get("controls").is_some(), true]
}
