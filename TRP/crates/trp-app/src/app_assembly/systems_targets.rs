//! The systems of the targets, the slung weapons, the watch, the bullets, the workbench and the dummies.

use super::*;

pub(super) struct TargetInputs<'a> {
    pub(super) bullets: Option<weapons::bullets::Bullets>,
    pub(super) with_poppers: bool,
    pub(super) slung: Option<player::slung_view::Slung>,
    pub(super) wrist_watch: Option<player::watch_view::WristWatch>,
    pub(super) sound_install: &'a Install,
    pub(super) impact_names: &'a Vec<String>,
    pub(super) player_armour: Option<effects::blast_view::PlayerArmour>,
    pub(super) cracks_on: bool,
    pub(super) fixed_step: f32,
    pub(super) dummy_source: Option<world::dummy::DummySource>,
    pub(super) dummy_lights_on: bool,
}

/// Adds the reactive targets, the slung weapons, the watch, the bullets, the workbench, the armour, the shields' marks and the dummies, with their systems' order.
pub(super) fn add(mut app: App, inputs: TargetInputs) -> App {
    let TargetInputs { bullets, with_poppers, slung, wrist_watch, sound_install, impact_names, player_armour, cracks_on, fixed_step, dummy_source, dummy_lights_on } = inputs;
    // The reactive targets (`REACTIVE TARGETS`), met by the bullets: read
    // as the run begins and moved after the frame's bullets (`poppers`).
    if bullets.is_some() && with_poppers {
        app.init_resource::<world::poppers::Poppers>();
        app.add_systems(Update, (world::poppers::read_poppers, world::poppers::place_poppers).chain());
        app.add_systems(Update, world::poppers::move_poppers.after(weapons::bullets::fly_bullets));
    }
    // The weapons on the body, with their switch on (`slung_view`).
    if let Some(slung) = slung {
        app.insert_resource(slung);
    }
    // The watch on the wrist, hung and turned once the arms are shown or
    // hidden for the frame (`watch_view`).
    if let Some(wrist_watch) = wrist_watch {
        app.insert_resource(wrist_watch);
        app.add_systems(Update, (player::watch_view::hang_watch, player::watch_view::turn_watch).chain().after(player::body_view::show_upper_body).after(player::death::follow));
    }
    if let Some(bullets) = bullets {
        app.insert_resource(bullets);
        audio::impact_sound::start_reading(&mut app, sound_install.clone(), impact_names.clone());
    }
    // The workbench's model, read as the run begins (`workbench`).
    app.init_resource::<world::workbench::WorkbenchModel>().init_resource::<world::workbench::WorkbenchAsked>();
    app.add_systems(Update, (world::workbench::read_workbench, world::workbench::place_workbench).chain());
    if let Some(armour) = player_armour {
        app.insert_resource(armour);
    }
    // The marks made after every hit and blast of the frame, before the
    // camera hears the shields (`shield_marks`; nothing without the switch).
    if cracks_on {
        app.insert_resource(world::shield_marks::ShieldMarks::new(fixed_step));
    }
    app.add_systems(
        Update,
        world::shield_marks::mark_shields
            .after(weapons::bullets::fly_bullets)
            .after(effects::blast_view::grenade_blasts)
            .after(effects::contusion_view::update),
    );
    // The weapons on the other bodies (`slung_view::hang_others`: the
    // figure's, the dummies'), after the character's and the inventory's
    // moves; with the dummies, after each is posed, falls, stands up or
    // loses its weapon (its holster bone placed after its pose).
    let others = player::slung_view::hang_others.after(player::slung_view::hang).after(screens::inventory_view::ScreenDrawn).run_if(resource_exists::<player::slung_view::Slung>);
    if dummy_source.is_some() {
        app.add_systems(Update, others.after(world::dummy::run_dummies).after(world::dummy::fall_dummies).after(world::dummy::heal_dummies).after(world::dummy::clear_dummy_weapons));
    } else {
        app.add_systems(Update, others);
    }
    if let Some(source) = dummy_source {
        app.insert_resource(source);
        // After the character's frame, whose knife meets the colliders where
        // the last frame placed them (the game's order as read: a knife's
        // sweep runs in the character's update, before the engine's
        // animation writes the others' bones;
        // `PROVISIONAL`, which pose the engine's copy of a collider holds
        // not settled); the knife's blows taken before the dummies step
        // (`blow_dummies`). After the character's camera is placed for the
        // frame, which the dummies' posing by their size on screen goes by.
        app.add_systems(Update, (world::dummy::make_dummies, world::dummy::run_dummies, world::dummy::place_dummies).chain().after(player::controller::step).after(player::controller::apply).before(weapons::bullets::fly_bullets));
        app.add_systems(Update, world::dummy::blow_dummies.after(player::controller::step).after(world::dummy::make_dummies).before(world::dummy::run_dummies));
        // Read after the screen's keys, which a capture's taps press.
        app.add_systems(Update, world::dummy::heal_dummies.after(screens::inventory_view::ScreenDrawn).after(world::dummy::make_dummies));
        app.add_systems(Update, world::dummy::show_dummies.after(world::dummy::heal_dummies));
        // A standing dummy's load read again after the screen's moves and
        // its frame's step (the next step the first to go by it).
        app.add_systems(Update, world::dummy::reweigh_dummies.after(screens::inventory_view::ScreenDrawn).after(world::dummy::heal_dummies).after(world::dummy::run_dummies));
        app.add_systems(Update, world::dummy::fall_dummies.after(weapons::bullets::fly_bullets).after(player::death::die));
        // Each dummy's armour's wear onto its things after the frame's
        // bullets and blasts; a dead body's weapon gone once its thing left
        // its slot, after the falls and the screen's moves.
        app.add_systems(Update, world::dummy::keep_dummy_wear.after(weapons::bullets::fly_bullets).after(world::dummy::blast_dummies).after(world::dummy::shot_blast_dummies));
        app.add_systems(Update, world::dummy::clear_dummy_weapons.after(world::dummy::fall_dummies).after(screens::inventory_view::ScreenDrawn));
        // On the blast's frame, its dummies moved and their bones as the
        // last frame posed them, as the game's blast finds its bots.
        app.add_systems(Update, world::dummy::blast_dummies.after(world::loose::fly).after(world::dummy::run_dummies).before(world::dummy::place_dummies).run_if(running));
        // An explosive round's, in the bullets' frame, before a death falls.
        app.add_systems(Update, world::dummy::shot_blast_dummies.after(weapons::bullets::fly_bullets).before(world::dummy::fall_dummies).before(world::shield_marks::mark_shields).run_if(running));
        app.add_systems(Update, world::dummy::stand_in_the_way.before(player::controller::step));
        app.insert_resource(world::dummy::DummyLights { on: dummy_lights_on }).add_systems(Update, world::dummy::switch_dummy_lights.after(screens::inventory_view::ScreenDrawn));
    }
    app
}
