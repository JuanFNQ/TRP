//! The engine's and the app's plugins, with the resources they begin with.

use super::*;

pub(super) struct PluginInputs<'a> {
    pub(super) options: Options,
    pub(super) player: ClipPlayer,
    pub(super) range: world::range::Range,
    pub(super) clothing: player::body_view::BodyParts,
    pub(super) plugins: bevy::app::PluginGroupBuilder,
    pub(super) tactical_on: bool,
    pub(super) range_own_light: bool,
    pub(super) waiting: &'a PipelinesWaiting,
    pub(super) updates: bevy::winit::WinitSettings,
    pub(super) sight_panels: panels::sight_panels::SightPanelsLook,
    pub(super) health_words: screens::health_view::Words,
    pub(super) health_look: screens::health_view::HealthLook,
    pub(super) ragdoll: player::death::Ragdoll,
    pub(super) physics_clock: player::death::PhysicsClock,
    pub(super) start_of_run: player::death::StartOfRun,
    pub(super) pointer_pictures: Vec<trp_data::interface::screen::CursorPicture>,
}

/// Adds the engine's default plugins and the app's own, with the resources they begin with; the dummies' lights' switch handed back.
pub(super) fn add(mut app: App, inputs: PluginInputs) -> (App, bool) {
    let PluginInputs { options, player, range, clothing, plugins, tactical_on, range_own_light, waiting, updates, sight_panels, health_words, health_look, ragdoll, physics_clock, start_of_run, pointer_pictures } = inputs;
    // A crash asked for, to see its report (`--crash`, `run_log`).
    if options.crash {
        app.add_systems(Update, platform::run_log::crash_on_purpose);
    }
    // The interface written out frame by frame, where asked (`--dump-ui`);
    // nothing of it runs otherwise.
    app.add_plugins(ui::ui_dump::UiDumpPlugin { path: options.dump_ui.clone(), from: options.dump_ui_from });
    let dummy_lights_on = options.dummy_lights;
    app.insert_resource(options)
        .insert_resource(player)
        .init_resource::<assets::render_data::Converted>()
        .insert_resource(range)
        .insert_resource(clothing)
        .add_plugins(player::clothing::ClothingPlugin)
        .add_plugins(assets::worn_reads::WornReadsPlugin)
        .add_plugins(screens::edit_build::EditBuildPlugin)
        .add_plugins(screens::info_preview::InfoPreviewPlugin)
        .add_plugins(screens::overall_view::OverallPlugin)
        .add_plugins(screens::skills_view::SkillsPlugin)
        .init_resource::<player::body_view::KeptCollarbones>()
        .init_resource::<player::body_view::OutfitOn>()
        .init_resource::<player::body_view::OutfitPose>()
        .add_plugins(plugins)
        .add_plugins(world::grid::GridPlugin)
        // The game's own surface for pictures and the inspect window's
        // preview (`game_surface`).
        .add_plugins(rendering::game_surface::GameSurfacePlugin)
        // And for the OVERALL tab's own figure (`figure_surface`).
        .add_plugins(screens::figure_surface::FigureSurfacePlugin)
        .add_plugins(rendering::shadow_only::ShadowOnlyPlugin)
        // What is hidden kept out of the engine's work (`park`).
        .add_plugins(platform::park::ParkPlugin)
        .add_plugins(assets::lod_view::LodPlugin)
        // The interface laid out only when something it is laid out from
        // has changed (`ui_layout`).
        .add_plugins(ui::ui_layout::UiLayoutPlugin)
        .add_plugins(platform::unready::UnreadyPlugin)
        .add_plugins(ui::radial_fill::RadialFillPlugin)
        .add_plugins(ui::screen_blur::ScreenBlurPlugin)
        // The game's screens shown softly, their whole groups faded in.
        .add_plugins(ui::screen_fade::ScreenFadePlugin)
        .add_plugins(effects::contusion_view::ContusionViewPlugin)
        .add_plugins(rendering::sharpen_view::SharpenViewPlugin)
        .add_plugins(effects::blood_view::BloodViewPlugin)
        .add_plugins(effects::visor_view::VisorViewPlugin)
        .add_plugins(weapons::tactical_view::TacticalViewPlugin { on: tactical_on, own_light: range_own_light })
        .add_plugins(effects::distort_view::DistortViewPlugin)
        .add_systems(Update, ui::screen_blur::fade.after(screens::inventory_view::ScreenKeys))
        .init_resource::<player::feet_down::FeetDown>()
        .insert_resource(FrameMark(std::time::Instant::now()))
        .init_resource::<Watching>()
        .init_resource::<platform::keys::ScriptedMouse>()
        .insert_resource(Warmup(true))
        .init_resource::<assets::render_data::SharedPictures>()
        .insert_resource(waiting.clone())
        .insert_resource(updates)
        .add_plugins(panels::hud::HudPlugin)
        .add_plugins(platform::debug_menu::DebugMenuPlugin)
        .add_plugins(player::gestures::GesturesPlugin)
        .add_plugins(screens::examine_view::ExaminePlugin)
        .add_plugins(player::statistics::StatisticsPlugin)
        .add_plugins(player::experience::ExperiencePlugin)
        .add_plugins(panels::using_view::UsingPlugin)
        .add_plugins(screens::search_view::SearchPlugin)
        .add_plugins(screens::inventory_view::InventoryPlugin)
        .add_plugins(ui::tooltip::TooltipPlugin)
        .add_plugins(screens::item_menu_view::ItemMenuPlugin)
        .add_plugins(audio::hands_sound::HandsSoundPlugin)
        .add_plugins(audio::shot_sound::ShotSoundPlugin)


        .add_plugins(screens::health_view::HealthViewPlugin)
        .add_plugins(panels::battle_view::BattleViewPlugin)
        .add_plugins(weapons::wear::WearPlugin)
        .add_plugins(weapons::kept_settings::KeptSettingsPlugin)
        .add_plugins(weapons::heat_view::HeatViewPlugin)
        .add_plugins(screens::repair_view::RepairViewPlugin)
        .add_plugins(panels::action_view::ActionViewPlugin)
        .add_plugins(panels::sight_panels::SightPanelsPlugin)
        .insert_resource(sight_panels)
        .add_plugins(audio::ui_sound::UiSoundPlugin)
        .add_plugins(audio::step_sound::StepSoundPlugin)
        .insert_resource(health_words)
        .insert_resource(health_look)
        .add_plugins(screens::options_view::OptionsPlugin)
        .add_plugins(screens::icons::IconsPlugin)
        .add_plugins(screens::figure_view::FigurePlugin)
        // A death: its corpse in the game's own engine (`trp_physics`).
        .add_plugins(player::death::DeathPlugin)
        .insert_resource(ragdoll)
        .insert_resource(physics_clock)
        .insert_resource(start_of_run)
        .insert_resource(platform::cursor::Pointers::of(pointer_pictures));
    (app, dummy_lights_on)
}
