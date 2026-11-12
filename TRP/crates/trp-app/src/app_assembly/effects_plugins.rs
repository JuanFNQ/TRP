//! The plugins of the shots' effects, the hands' sounds, and the sounds' log and tape.

use super::*;

pub(super) struct EffectsInputs {
    pub(super) muzzle_drawn: bool,
    pub(super) range_own_light: bool,
    pub(super) particles_drawn: bool,
    pub(super) tracers_drawn: bool,
    pub(super) tactical_on: bool,
    pub(super) impact_names: Vec<String>,
    pub(super) sound_install: Install,
    pub(super) hands_sounds: bool,
}

/// Adds the effects' plugins and what they read, the hands' sounds, and the sounds' log and tape where asked.
pub(super) fn add(mut app: App, inputs: EffectsInputs) -> App {
    let EffectsInputs { muzzle_drawn, range_own_light, particles_drawn, tracers_drawn, tactical_on, impact_names, sound_install, hands_sounds } = inputs;
    // The flash's pass is always in the graph (the gamma passes go round
    // it); with `MUZZLE FLASH` off nothing feeds it.
    app.add_plugins(weapons::muzzle_view::MuzzleViewPlugin { on: muzzle_drawn, own_light: range_own_light });
    // Drawn by the flash's pass, before its jets.
    app.add_plugins(effects::particles_view::ParticlesViewPlugin { on: particles_drawn });
    // Drawn by the flash's pass too, after its jets (`TRACERS`, a part of
    // `VISUAL EFFECTS`, with `BALLISTICS`).
    app.add_plugins(weapons::tracer_view::TracerViewPlugin { on: tracers_drawn });
    // The lights' flares (`TACTICAL DEVICES`): the dummies' lights' and
    // one's own (seen from outside).
    app.add_plugins(weapons::flare_view::FlareViewPlugin { on: tactical_on });
    // A grenade's explosion, on the frame before the muzzle's pass.
    app.add_plugins(effects::explosion_view::ExplosionViewPlugin { on: particles_drawn });
    // A grenade's scorch, with the explosions and the hard settings'
    // deferred decals (`decal_view`).
    let deferred_decals = app.world().get_resource::<GameData>().is_some_and(|game| game.hard.float("DEFERRED_DECALS_ENABLED").is_ok_and(|on| on != 0.0));
    app.add_plugins(effects::decal_view::DecalPlugin { on: particles_drawn && deferred_decals });
    // What those four draw, read away from the frame (`effects_reads`).
    app.add_plugins(assets::effects_reads::EffectsReadsPlugin(assets::effects_reads::EffectsWanted { particles: particles_drawn, tracers: tracers_drawn, scorches: particles_drawn && deferred_decals, impact_names }));
    audio::hands_sound::start_reading(&mut app, sound_install, hands_sounds);
    if let Some(path) = app.world().resource::<Options>().log_sounds.clone() {
        match std::fs::File::create(&path) {
            Ok(file) => {
                app.insert_resource(audio::step_sound::SoundLog(std::sync::Mutex::new(std::io::BufWriter::new(file))));
            }
            Err(e) => eprintln!("the sounds' log could not be made at {}: {e}", path.display()),
        }
    }
    if app.world().resource::<Options>().sound_out.is_some() {
        app.world().resource::<audio::ui_sound::Mixer>().0.start_tape();
        app.add_systems(Last, write_tape);
    }
    app
}
