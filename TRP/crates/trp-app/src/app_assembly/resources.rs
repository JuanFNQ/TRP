//! The app made, and the resources it begins with: the controller, the inventory's state, the game's data.

use super::*;

pub(super) struct ResourceInputs<'a> {
    pub(super) screens_coming: Option<std::thread::JoinHandle<(screens::health_view::HealthLook, std::collections::HashMap<String, String>)>>,
    pub(super) options: &'a Options,
    pub(super) controller: Controller,
    pub(super) equipment: trp_sim::items::inventory::Equipment,
    pub(super) mastery: player::skills::Mastery,
    pub(super) names: std::collections::HashMap<String, String>,
    pub(super) loot: trp_sim::items::inventory::Item,
    pub(super) away: Vec<Option<trp_sim::items::inventory::Item>>,
    pub(super) repair_settings: std::sync::Arc<trp_sim::items::repair::Settings>,
    pub(super) rules: trp_sim::items::inventory::Rules,
    pub(super) fixed_wear: Vec<String>,
    pub(super) panel: trp_data::interface::screen::GearPanel,
    pub(super) ground: Vec<(trp_sim::items::inventory::Item, Vec3)>,
    pub(super) buffs: trp_sim::condition::skills::Buffs,
    pub(super) texts: trp_data::character::inventory::ItemTexts,
    pub(super) named_builds: Vec<(String, Vec<String>)>,
    pub(super) searcher: trp_sim::items::search::Searcher,
    pub(super) loose: world::loose::Loose,
    pub(super) hard: HardSettings,
    pub(super) options_state: screens::options_view::OptionsState,
    pub(super) textures: assets::streaming::TextureSettings,
    pub(super) bindings: platform::keys::Bindings,
    pub(super) unity: UnityAssets,
    pub(super) db: std::sync::Arc<Database>,
    pub(super) skills: player::skills::Skills,
    pub(super) earning: player::experience::Experience,
    pub(super) engine: EngineSettings,
    pub(super) hands_data: Option<HandsData>,
    pub(super) words: std::sync::Arc<std::collections::HashMap<String, String>>,
    pub(super) unfinished: std::sync::Arc<std::collections::HashSet<String>>,
}

/// Makes the app and inserts the resources it begins with.
pub(super) fn insert(inputs: ResourceInputs) -> App {
    let ResourceInputs { screens_coming, options, controller, equipment, mastery, names, loot, away, repair_settings, rules, fixed_wear, panel, ground, buffs, texts, named_builds, searcher, loose, hard, options_state, textures, bindings, unity, db, skills, earning, engine, hands_data, words, unfinished } = inputs;
    let mut app = App::new();
    if let Some(thread) = screens_coming {
        app.insert_resource(screens::health_view::ScreensComing(Some(thread)));
    }
    if let Some(seconds) = options.figure_hold {
        app.insert_resource(screens::figure_view::FigureHold(seconds));
    }
    if !options.figure_picks.is_empty() {
        app.insert_resource(screens::figure_view::FigurePicks(options.figure_picks.clone()));
    }
    if let Some(ms) = options.fixed_dt {
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(ms / 1000.0)));
    }
    let [r, g, b] = rendering::lighting::RANGE_OWN_SKY;
    app.insert_resource(ClearColor(Color::srgb(r, g, b)))
        .insert_resource(GlobalAmbientLight { color: Color::WHITE, brightness: rendering::lighting::RANGE_OWN_AMBIENT, ..default() })
        .insert_resource(controller)
        .add_message::<world::loose::Blasted>()
        .add_message::<audio::tinnitus::Contused>()
        .insert_resource(screens::inventory_view::Carried {
            checked: screens::inventory_view::magazines_in(&equipment.item),
            checked_chambers: Default::default(),
            mastery: mastery.clone(),
            slot_keys: Vec::new(),
            equipment,
            names,
            loot,
            loot_open: false,
            away,
            loot_of: 0,
            searched_ground: None,
            repairing: false,
            repairs: Vec::new(),
            repair_settings: Some(repair_settings),
            rules,
            fixed_wear,
            note: String::new(),
            panel,
            ground,
            icons: Default::default(),
            pictures: Default::default(),
            pictures_on: options_state.in_force.icon_generation,
            picture_failed: Default::default(),
            spinner: Handle::default(),
            bound: Default::default(),
            top_grenade: None,
            magazine_work: None,
            drained: None,
            leaving: Vec::new(),
            frozen_keys: Default::default(),
            reload_key: None,
            warning: None,
            switched: Vec::new(),
            fold_asked: None,
            round_times: screens::inventory_view::RoundTimes { drills: buffs.mag_drills, ..screens::inventory_view::RoundTimes::of(&db) },
            strength_elite: buffs.strength_elite,
            carry: 1.0 + buffs.strength_lift,
            weighed: Default::default(),
            told_weight: 0.0,
            windows: Vec::new(),
            priority: None,
            // The game's setting for which window Ctrl and a click sends
            // things to, as the install has it (`Auto` where it has none:
            // what the install first read had).
            priority_auto: unity.install.game_setting("PriorityWindowMode").is_none_or(|mode| mode == "Auto"),
            asking: None,
            pack_asked: false,
            falling: Vec::new(),
            thrown: Vec::new(),
            item_looks: Default::default(),
            texts,
            named_builds,
            info_size: None,
            game_grid_window: false,
            tag_size: None,
            tag_limit: crate::screens::health_view::TAG_LIMIT,
            tag_edits: Vec::new(),
            observed: Vec::new(),
            wishlist: Default::default(),
            examiner: Default::default(),
            examine_asked: None,
            searcher: searcher.clone(),
            search_asked: None,
            search_clock: 0.0,
            search_timer_said: None,
            bodies: Vec::new(),
            searched_body: None,
            search_double: buffs.search_double,
        })
        .insert_resource(loose)
        .insert_resource(weapons::cases::Cases::new(&hard, options_state.in_force.spent_cases))
        .init_resource::<weapons::magazine_rounds::ChamberKinds>()
        .init_resource::<screens::ammo_picker::Picker>()
        // The models' pictures by their levels of detail (`streaming`).
        .insert_resource(assets::streaming::MipStreaming::new(textures))
        .insert_resource(bindings)
        .insert_resource(screens::icons::Icons::new(unity.install.clone(), db.clone(), options_state.in_force.icon_generation, screens::icons::signatures(&db)))
        .insert_resource(options_state.in_force.clone())
        .insert_resource(options_state.game)
        .insert_resource(options_state.sound)
        .insert_resource(options_state)
        .insert_resource(skills)
        .insert_resource(mastery)
        .insert_resource(earning)
        .insert_resource(assets::worn_reads::WornReads::new(unity.install.clone(), db.clone()))
        .insert_resource(GameData { unity, db, hard, engine, hands_data, words, unfinished })
        .insert_resource(options.view);
    app
}
