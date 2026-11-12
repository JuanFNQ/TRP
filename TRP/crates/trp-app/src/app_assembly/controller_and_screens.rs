//! The controller with its health, the battle screen's panels waited for, the pointers and the repair's figures.

use super::*;

pub(super) struct ControllerInputs<'a> {
    pub(super) movement: trp_sim::Movement,
    pub(super) options: &'a Options,
    pub(super) engine: &'a EngineSettings,
    pub(super) camera_box: Option<trp_data::character::player::CameraBoxData>,
    pub(super) options_state: &'a screens::options_view::OptionsState,
    pub(super) hard: &'a HardSettings,
    pub(super) db: &'a std::sync::Arc<Database>,
    pub(super) skills: &'a player::skills::Skills,
    pub(super) screens_reading: super::graphics::ScreensReading,
    pub(super) unity: &'a UnityAssets,
    pub(super) words: &'a std::sync::Arc<std::collections::HashMap<String, String>>,
    pub(super) panel: &'a trp_data::interface::screen::GearPanel,
}

pub(super) struct ControllerAndScreens {
    pub(super) controller: Controller,
    pub(super) health_look: screens::health_view::HealthLook,
    pub(super) sight_panels: panels::sight_panels::SightPanelsLook,
    pub(super) screens_coming: Option<std::thread::JoinHandle<(screens::health_view::HealthLook, std::collections::HashMap<String, String>)>>,
    pub(super) pointer_pictures: Vec<trp_data::interface::screen::CursorPicture>,
    pub(super) repair_settings: std::sync::Arc<trp_sim::items::repair::Settings>,
    pub(super) health_words: screens::health_view::Words,
}

/// Makes the controller and the character's health, waits for the battle screen's part, and reads the pointers, the repair's figures and the health panel's words.
pub(super) fn make(inputs: ControllerInputs) -> anyhow::Result<ControllerAndScreens> {
    let ControllerInputs { movement, options, engine, camera_box, options_state, hard, db, skills, screens_reading, unity, words, panel } = inputs;
    let mut controller =
        Controller::new(movement, trp_sim::Pacer::new(options.frame_rate, engine.fixed_timestep, engine.maximum_timestep));
    controller.camera_box = camera_box;
    // The simulation's systems as the options' switches have them for
    // this run (settings spec, "Features").
    controller.movement.systems = options_state.in_force.systems();
    if !controller.movement.systems.weight {
        controller.movement.set_weight(&hard, &db.config, 0.0)?;
    }
    // The character's health, with the options' `HEALTH SYSTEM` on: full,
    // as every run begins, its chances drawn
    // from the run's seed (`--seed`).
    if options_state.in_force.health {
        match trp_sim::condition::health::HealthSettings::new(&db.config) {
            Ok(settings) => {
                let mut health = trp_sim::condition::health::Health::new(std::sync::Arc::new(settings), options.seed);
                // Its skills' buffs (`skills`).
                health.skills = skills.health();
                controller.health = Some(health);
            }
            Err(e) => eprintln!("the install's figures for health could not be read; the character cannot be hurt: {e:#}"),
        }
    }
    // The game's own health panel for the inventory's HEALTH tab, read
    // from the scene that has its equipment panel: its layout, where the
    // app's own look puts its boxes too, and with `TARKOV UI` on its
    // pictures and clips.
    platform::run_log::phase("the controller");
    // The battle screen's part and the sights' panels, waited for; the rest
    // comes during the warm-up (`health_view::ScreensComing`).
    let battle_part = screens_reading.and_then(|(thread, battle_part)| match battle_part.recv() {
        Ok(part) => Some((part, thread)),
        Err(_) => {
            eprintln!("the screens' thread failed before the battle screen's part, and they are read on this one");
            None
        }
    });
    let (mut health_look, sight_panels, screens_coming) = match battle_part {
        Some(((look, sights), thread)) => (look, sights, Some(thread)),
        None => {
            let layouts: Vec<String> = db.items.values().map(|item| item.grid_layout_name().to_owned()).filter(|name| !name.is_empty()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
            let mut battle = None;
            let mut look = screens::health_view::HealthLook::read(&unity.install, &words, &panel.file, options_state.in_force.tarkov_ui, screens::health_view::weight_limits(&db.config), &layouts, options_state.in_force.clothing, options_state.in_force.workbench || options_state.in_force.armor_bench, |part| battle = Some(part));
            if let Some(battle) = battle {
                look.rejoin(battle);
            }
            (look, panels::sight_panels::SightPanelsLook::read(&unity.install, &panel.file), None)
        }
    };
    platform::run_log::phase("the battle screen's panels and the sights', waited for");
    // The game's own mouse pointers, with `TARKOV UI` (`cursor::point`).
    let pointer_pictures = if options_state.in_force.tarkov_ui {
        trp_data::interface::screen::cursor_pictures(&unity.install).unwrap_or_else(|e| {
            warn!("the game's mouse pointers could not be read; the system's stays: {e:#}");
            Vec::new()
        })
    } else {
        Vec::new()
    };
    // The repair's figures (`trp_sim::repair`): the globals' and the
    // hideout's areas' greatest bonuses.
    let areas = std::fs::read_to_string(unity.install.database_dir().join("hideout/areas.json")).ok().and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok());
    let mut repair_settings = trp_sim::items::repair::Settings::from_config(&db.config, areas.as_ref());
    // The buffs a kit's repair may bring (SPT's `configs/repair.json`).
    match trp_data::level::loot::spt_config(&unity.install, "repair.json") {
        Ok(config) => repair_settings.kit_buffs = trp_sim::items::repair::KitBuffs::from_config(&config),
        Err(e) => warn!("SPT's repair.json could not be read; a repair brings no buff: {e:#}"),
    }
    let repair_settings = std::sync::Arc::new(repair_settings);
    health_look.level_experience = screens::health_view::level_experience(&db.config);
    health_look.skill_effectiveness = screens::skills_view::effectiveness(&db.config, 0);
    platform::run_log::phase("the mouse pointers, the repair's figures");
    let health_words = screens::health_view::Words::load(&words, &health_look.keys());
    platform::run_log::phase("the health panel's words");
    Ok(ControllerAndScreens { controller, health_look, sight_panels, screens_coming, pointer_pictures, repair_settings, health_words })
}
