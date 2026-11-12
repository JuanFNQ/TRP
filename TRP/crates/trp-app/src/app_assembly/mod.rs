//! The app assembled: its plugins, resources and systems.

use super::*;

mod install_options;
mod character_kit;
mod item_texts;
mod switches;
mod range_things;
mod bundles_and_skills;
mod graphics;
mod dummies;
mod worn;
mod body;
mod level;
mod renderer;
mod controller_and_screens;
mod loose_things;
mod world_light;
mod effects_and_bullets;
mod dummy_source;
mod resources;
mod engine_plugins;
mod systems_frame;
mod systems_targets;
mod render_streaming;
mod systems_sounds;
mod effects_plugins;
mod camera_passes;

pub(super) fn run() -> anyhow::Result<()> {
    let Some((install, mut options)) = install_options::open()? else {
        return Ok(());
    };
    let character_kit::Kit { db, fixed_wear, mut equipment } = character_kit::load(&install, &mut options)?;
    let item_texts::Texts { locale, words, texts, named_builds, names, panel, rules, loot } = item_texts::read(&install, &db)?;
    let switches::Switches { bindings, families, mut options_state, unfinished } = switches::read(&install, &mut options, &words, &db);
    let range_things::RangeThings { mut away, mut ground, laid_from } = range_things::lay_out(&db, &install, &unfinished, &mut options, &mut equipment, &rules)?;
    let bundles_and_skills::BundlesAndSkills { mut unity, hard, engine, movement_settings, movement_as_read, earning, skills, mastery, buffs, rounds_drawn, rules, muzzle_drawn, particles_drawn, tracers_drawn } =
        bundles_and_skills::load(install, &db, &mut options, &mut options_state, &equipment, families, rules)?;
    let graphics::Graphics { screens_reading, textures, limiter } = graphics::settings(&unity, &panel, &options_state, &db, &words, &mut options);
    let dummies::Dummies { with_dummy, dummy_kits, dummy_looks, dummy_voice_seed, dummy_voices } = dummies::pick(&options_state, &options, &unity, &db);
    let worn::Worn { dummy_kits, present, wardrobe, slung, slung_bones, wrist_watch } = worn::read(&equipment, dummy_kits, &mut options, &options_state, &mut unity, locale, &db, &dummy_looks);
    let body::BodyAndMovement { clothing, with_poppers, scope_pictures, boot_shadow, body_data, mut movement, hands_data } = body::load(body::BodyInputs {
        unity: &mut unity,
        db: &db,
        options: &options,
        present: &present,
        options_state: &options_state,
        slung_bones: &slung_bones,
        dummy_looks: &dummy_looks,
        with_dummy,
        movement_settings,
        buffs: &buffs,
        skills: &skills,
        hard: &hard,
    })?;
    let Some(level::RangeSet { range, camera_box }) = level::lay_out(&options, &options_state, &mut unity, &hard, &mut movement, &mut ground, laid_from)? else {
        return Ok(());
    };
    let renderer::RendererSettings { player, updates, plugins } = renderer::settings(&options, &options_state);
    let controller_and_screens::ControllerAndScreens { controller, health_look, sight_panels, screens_coming, pointer_pictures, repair_settings, health_words } =
        controller_and_screens::make(controller_and_screens::ControllerInputs {
            movement,
            options: &options,
            engine: &engine,
            camera_box,
            options_state: &options_state,
            hard: &hard,
            db: &db,
            skills: &skills,
            screens_reading,
            unity: &unity,
            words: &words,
            panel: &panel,
        })?;
    let loose_things::LooseThings { ragdoll, loose, physics_clock, searcher, start_of_run } = loose_things::make(loose_things::LooseInputs {
        unity: &mut unity,
        hard: &hard,
        db: &db,
        options_state: &options_state,
        options: &options,
        rounds_drawn,
        away: &mut away,
        ground: &ground,
        laid_from,
        controller: &controller,
        equipment: &equipment,
        loot: &loot,
        range: &range,
    });
    let world_light::WorldLight { filter_table, level_lighting, own_sharpening, game_shading, figure_shading, boot_shadow, range_own_light } = world_light::read(&options_state, &options, &unity, boot_shadow);
    let effects_and_bullets::EffectsAndBullets {
        waiting,
        gamma_composed,
        sound_install,
        day_install,
        movement_sounds,
        world_sounds,
        hands_sounds,
        headgear_hearing,
        grenades,
        ballistics_on,
        health_on,
        visor_on,
        cracks_on,
        tactical_on,
        impact_names,
        bullets,
    } = effects_and_bullets::read(&options_state, &mut unity, &hard, &db, &engine);
    let dummy_source::DummiesMade { dummy_source, player_armour } = dummy_source::make(dummy_source::DummySourceInputs {
        options_state: &options_state,
        bullets: &bullets,
        unity: &mut unity,
        db: &db,
        hard: &hard,
        engine: &engine,
        movement_as_read: &movement_as_read,
        body_data: &body_data,
        options: &options,
        dummy_kits,
        grenades,
    });
    let fixed_step = engine.fixed_timestep;
    // What the character knows (`EXAMINING`; inventory spec, "Examining"):
    // the new character's list it is made from, `--loadout`'s or the
    // Standard BEAR's, with what was learned before; and what it carries
    // now.
    {
        let (edition, side) = options.loadout.as_deref().map_or(("Standard", "bear"), |loadout| loadout.split_once('/').unwrap_or((loadout, "usec")));
        screens::examine_view::begin_run(&unity.install, edition, side, options_state.in_force.examining);
        screens::examine_view::know_carried(&equipment.item);
    }
    // The character's own equipment, apart from a body's (`Rules::own`).
    platform::run_log::phase("the bullets, the dummies' source");
    let rules = trp_sim::items::inventory::Rules { own: Some(equipment.item.id), ..rules };
    let app = resources::insert(resources::ResourceInputs {
        screens_coming,
        options: &options,
        controller,
        equipment,
        mastery,
        names,
        loot,
        away,
        repair_settings,
        rules,
        fixed_wear,
        panel,
        ground,
        buffs,
        texts,
        named_builds,
        searcher,
        loose,
        hard,
        options_state,
        textures,
        bindings,
        unity,
        db,
        skills,
        earning,
        engine,
        hands_data,
        words,
        unfinished,
    });
    let (app, dummy_lights_on) = engine_plugins::add(app, engine_plugins::PluginInputs {
        options,
        player,
        range,
        clothing,
        plugins,
        tactical_on,
        range_own_light,
        waiting: &waiting,
        updates,
        sight_panels,
        health_words,
        health_look,
        ragdoll,
        physics_clock,
        start_of_run,
        pointer_pictures,
    });
    let app = systems_frame::add(app);
    let app = systems_targets::add(app, systems_targets::TargetInputs {
        bullets,
        with_poppers,
        slung,
        wrist_watch,
        sound_install: &sound_install,
        impact_names: &impact_names,
        player_armour,
        cracks_on,
        fixed_step,
        dummy_source,
        dummy_lights_on,
    });
    let app = render_streaming::add(app, waiting, wardrobe);
    let app = systems_sounds::add(app, systems_sounds::SoundInputs {
        sound_install: &sound_install,
        movement_sounds,
        world_sounds,
        grenades,
        ballistics_on,
        health_on,
        with_dummy,
        dummy_voices,
        dummy_voice_seed,
        headgear_hearing,
        visor_on,
        cracks_on,
        rounds_drawn,
    });
    let app = effects_plugins::add(app, effects_plugins::EffectsInputs { muzzle_drawn, range_own_light, particles_drawn, tracers_drawn, tactical_on, impact_names, sound_install, hands_sounds });
    let mut app = camera_passes::add(app, camera_passes::CameraInputs {
        filter_table,
        scope_pictures,
        game_shading,
        figure_shading,
        boot_shadow,
        limiter,
        range_own_light,
        day_install,
        level_lighting,
        own_sharpening,
        gamma_composed,
    });
    platform::run_log::phase("the engine's plugins added");
    app.run();
    Ok(())
}
