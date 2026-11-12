//! The screens' reading thread started, and the graphics' settings from textures to ambient occlusion.

use super::*;

/// The screens read on a thread of their own: its handle, and the battle screen's part as it comes.
pub(super) type ScreensReading = Option<(std::thread::JoinHandle<(screens::health_view::HealthLook, std::collections::HashMap<String, String>)>, std::sync::mpsc::Receiver<(screens::health_view::HealthLook, panels::sight_panels::SightPanelsLook)>)>;

pub(super) struct Graphics {
    pub(super) screens_reading: ScreensReading,
    pub(super) textures: assets::streaming::TextureSettings,
    pub(super) limiter: platform::frame_limit::FrameLimit,
}

/// Starts reading the screens away from the frame and settles the graphics' settings from the options and the command line.
pub(super) fn settings(unity: &UnityAssets, panel: &trp_data::interface::screen::GearPanel, options_state: &screens::options_view::OptionsState, db: &std::sync::Arc<Database>, words: &std::sync::Arc<std::collections::HashMap<String, String>>, options: &mut Options) -> Graphics {
    // The pictures' quality and streaming, as the options have them or the
    // command line says.
    platform::run_log::phase("the options and the skills");
    // The game's own health panel for the inventory's HEALTH tab and the
    // rest of its screens (`health_view::HealthLook::read`), and the battle
    // screen's panels of a sight: read on a thread of their own while the
    // body and the rest are made, since they read nothing those do (the
    // install's screens alone), and waited for where they are wanted below.
    // On the frame's thread they held the start for a second.
    let screens_reading = {
        let (install, file, pictures, limits) = (unity.install.clone(), panel.file.clone(), options_state.in_force.tarkov_ui, screens::health_view::weight_limits(&db.config));
        let (clothing, workbench) = (options_state.in_force.clothing, options_state.in_force.workbench || options_state.in_force.armor_bench);
        let layouts: Vec<String> = db.items.values().map(|item| item.grid_layout_name().to_owned()).filter(|name| !name.is_empty()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        let screen_words = words.clone();
        // The battle screen's part first, handed over as soon as it is read
        // with the sights' panels (the start waits for it below); then the
        // rest, the inventory's, and the install's words for its keys, merged
        // in when they come (`health_view::ScreensComing`), so that the
        // inventory's part is read in the background.
        let (started, battle_part) = std::sync::mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("the screens".to_owned())
            .spawn(move || {
                let rest = screens::health_view::HealthLook::read(&install, &screen_words, &file, pictures, limits, &layouts, clothing, workbench, |battle| {
                    let _ = started.send((battle, panels::sight_panels::SightPanelsLook::read(&install, &file)));
                });
                let keys = rest.keys();
                let names: Vec<&str> = keys.iter().map(String::as_str).collect();
                let texts = trp_data::character::inventory::words_of(&screen_words, &names);
                (rest, texts)
            })
            .map_err(|e| eprintln!("the screens' thread could not be started, and they are read on this one: {e}"))
            .ok()
            .map(|thread| (thread, battle_part))
    };
    let textures = assets::streaming::TextureSettings {
        quality: options.texture_quality.unwrap_or(options_state.graphics.texture_quality),
        streaming: options.mip_streaming.unwrap_or(options_state.graphics.mip_streaming),
    };
    println!(
        "pictures: texture quality {} of 2, mip streaming {}",
        textures.quality,
        if textures.streaming { format!("on (within {} MB, at most {} levels down)", assets::streaming::BUDGET >> 20, textures.most_reduction()) } else { format!("off ({} of every picture's levels left off)", textures.limit()) }
    );
    // Before any picture is made.
    assets::streaming::set(textures);
    // The sun's shadows, as the options have them or the command line says.
    let shadow_quality = options.shadow_quality.unwrap_or(options_state.graphics.shadow_quality).min(3);
    options.shadow_quality = Some(shadow_quality);
    println!("shadows: quality {}", rendering::shadows::describe(shadow_quality));
    // The ambient occlusion, as the options have it or the command line
    // says, with `AMBIENT OCCLUSION` (`hbao_view`).
    // The scene's multisampling and the frame rate limit, as the options
    // have them or the command line says (`frame_limit`).
    let msaa = options.msaa.unwrap_or(options_state.graphics.msaa);
    options.msaa = Some(msaa);
    // The dust's resolution, as the options have it or the command line
    // says (`offscreen_dust`), changed at once from the options from then on.
    let dust = options.dust_resolution.unwrap_or(options_state.graphics.dust_resolution);
    effects::offscreen_dust::set(dust);
    println!("dust resolution: {}", dust.key());
    let limiter = platform::frame_limit::FrameLimit::new(options.fps_limit, !options.unattended());
    println!(
        "msaa: {}; frame rate limit: {}",
        if msaa > 1 { format!("{msaa}x") } else { "off".to_owned() },
        match (options.fps_limit, options.unattended(), options_state.graphics.fps_limit_off) {
            (Some(Some(fps)), ..) => format!("{fps} a second with vsync off (--fps-limit)"),
            (Some(None), ..) => "off (--fps-limit)".to_owned(),
            (None, true, _) => "none (a run nobody is at the keys of)".to_owned(),
            (None, false, true) => "off (the options)".to_owned(),
            (None, false, false) => format!("{} a second with vsync off (the options)", options_state.graphics.game_fps),
        }
    );
    let ssao = options.ssao.unwrap_or(options_state.graphics.ssao);
    options.ssao = Some(ssao);
    match (options_state.in_force.ambient_occlusion, ssao.hbao()) {
        (false, _) => println!("ambient occlusion: off (AMBIENT OCCLUSION)"),
        (true, None) => println!("ambient occlusion: off (SSAO {})", ssao.key()),
        (true, Some(s)) => println!(
            "ambient occlusion: SSAO {}, HBAO {:?} ({} directions of {} steps), radius {} m, at most {} pixels, blur {:?}{}{}",
            ssao.key(),
            s.quality,
            s.quality.directions(),
            s.quality.steps(),
            s.radius,
            s.max_radius_pixels,
            s.blur,
            if s.blur_downsample { " at half size" } else { "" },
            if ssao.cinematic() { "; the game's second effect at this choice is not built" } else { "" }
        ),
    }
    Graphics { screens_reading, textures, limiter }
}
