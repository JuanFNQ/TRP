//! The sounds read away from the frame and their systems, the hearing, the sharpening, the visor and the magazines' rounds.

use super::*;

pub(super) struct SoundInputs<'a> {
    pub(super) sound_install: &'a Install,
    pub(super) movement_sounds: bool,
    pub(super) world_sounds: bool,
    pub(super) grenades: bool,
    pub(super) ballistics_on: bool,
    pub(super) health_on: bool,
    pub(super) with_dummy: bool,
    pub(super) dummy_voices: Vec<Option<String>>,
    pub(super) dummy_voice_seed: u64,
    pub(super) headgear_hearing: bool,
    pub(super) visor_on: bool,
    pub(super) cracks_on: bool,
    pub(super) rounds_drawn: bool,
}

/// Starts reading the sounds and adds their systems, the hearing, the sharpening's and the visor's frames, and the rounds' parity.
pub(super) fn add(mut app: App, inputs: SoundInputs) -> App {
    let SoundInputs { sound_install, movement_sounds, world_sounds, grenades, ballistics_on, health_on, with_dummy, dummy_voices, dummy_voice_seed, headgear_hearing, visor_on, cracks_on, rounds_drawn } = inputs;
    audio::ui_sound::start_reading(&mut app, sound_install.clone());
    // The SOUNDS page's voices heard, whatever the switches.
    audio::voice_sound::start_preview(&mut app, sound_install.clone());
    audio::step_sound::start_reading(&mut app, sound_install.clone(), movement_sounds);
    // The things' own sounds, after the range has stepped its things.
    audio::thing_sound::start_reading(&mut app, sound_install.clone(), world_sounds);
    app.add_systems(Update, audio::thing_sound::play_things.run_if(resource_exists::<audio::thing_sound::ThingSoundBanks>).after(player::controller::step));
    // A weapon heard as it lands, under the same switch.
    audio::landing_sound::start_reading(&mut app, sound_install.clone(), world_sounds);
    weapons::cases::start_reading(&mut app, sound_install.clone());
    // A thrown grenade's blasts, and an explosive round's (`BALLISTICS`).
    if grenades || ballistics_on {
        audio::blast_sound::start_reading(&mut app, sound_install.clone());
    }
    if health_on {
        audio::tinnitus::start_reading(&mut app, sound_install.clone());
        // The character's voice and a bone's crack.
        let voice = app.world().resource::<screens::options_view::OptionsState>().voice.clone();
        audio::voice_sound::start_reading(&mut app, sound_install.clone(), true, voice);
        // The dummies' voices, under `TARGETS` too (`dummy_voice`).
        world::dummy_voice::start(&mut app, sound_install.clone(), with_dummy, dummy_voices, dummy_voice_seed);
        app.init_resource::<effects::contusion_view::ContusionState>();
        app.init_resource::<effects::blood_view::BloodState>();
        effects::contusion_view::start_reading(&mut app, sound_install.clone());
    } else {
        // Off, nothing is read and nothing said; the phrases' messages and
        // the voiced phrases (none) still there for the gestures' menu,
        // whose systems stopped the app without them.
        audio::voice_sound::start_reading(&mut app, sound_install.clone(), false, None);
    }
    // The groups' levels as heard under what is worn on the head, after the
    // tinnitus' frame (with the switch off, the tinnitus' dip alone).
    audio::hearing::start(&mut app, headgear_hearing);
    // The camera's sharpening, after health's accumulators have run.
    app.add_systems(Update, rendering::sharpen_view::update.after(effects::contusion_view::update));
    if visor_on {
        effects::visor_view::start_reading(&mut app, sound_install.clone(), cracks_on);
        // After the screen's frame, so that a switch flipped in this frame
        // (the goggles' key before the controller's step, the menu, a part
        // fitted or taken off) is heard on it, as the game's camera hears it.
        app.add_systems(Update, effects::visor_view::visor_frame.after(effects::contusion_view::update).after(screens::inventory_view::ScreenDrawn).after(world::shield_marks::mark_shields));
    }
    if rounds_drawn {
        app.insert_resource(weapons::magazine_rounds::NextParity::default());
    }
    app
}
