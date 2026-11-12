//! The switches of the sounds and the effects as the run begins, the sounds' install, and the bullets.

use super::*;

pub(super) struct EffectsAndBullets {
    pub(super) waiting: PipelinesWaiting,
    pub(super) gamma_composed: bool,
    pub(super) sound_install: Install,
    pub(super) day_install: Install,
    pub(super) movement_sounds: bool,
    pub(super) world_sounds: bool,
    pub(super) hands_sounds: bool,
    pub(super) headgear_hearing: bool,
    pub(super) grenades: bool,
    pub(super) ballistics_on: bool,
    pub(super) health_on: bool,
    pub(super) visor_on: bool,
    pub(super) cracks_on: bool,
    pub(super) tactical_on: bool,
    pub(super) impact_names: Vec<String>,
    pub(super) bullets: Option<weapons::bullets::Bullets>,
}

/// Takes the sounds' and effects' switches for the run and sets up the bullets with the hits' effects.
pub(super) fn read(options_state: &screens::options_view::OptionsState, mut unity: &mut UnityAssets, hard: &HardSettings, db: &std::sync::Arc<Database>, engine: &EngineSettings) -> EffectsAndBullets {
    // Until the renderer has reported, it counts as having work to do.
    platform::run_log::phase("Customs' light, the sharpening");
    let waiting = PipelinesWaiting(std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(1)));
    // With `TARKOV UI` the screens are composed in gamma space, as the
    // game's are (`gamma_ui`).
    let gamma_composed = options_state.in_force.tarkov_ui;
    // The install, for the sounds read away from the frame (`ui_sound`,
    // `step_sound`), and whether the sounds of moving are wanted.
    let sound_install = unity.install.clone();
    let day_install = unity.install.clone();
    let movement_sounds = options_state.in_force.movement_sounds;
    let world_sounds = options_state.in_force.world_sounds;
    let hands_sounds = options_state.in_force.hands_sounds;
    // A helmet's cut of what is heard (`HEADGEAR HEARING`, needing `SOUNDS`).
    let headgear_hearing = options_state.in_force.headgear_hearing;
    let grenades = options_state.in_force.grenades_and_knife;
    let ballistics_on = options_state.in_force.ballistics;
    let health_on = options_state.in_force.health;
    // The face shield's mask over the view (`FACE SHIELD MASK`, a part of
    // `VISUAL EFFECTS`).
    let visor_on = options_state.in_force.face_shield_mask;
    // A face shield's marks and their cracks (`FACE SHIELD CRACKS`, beside
    // the mask among the effects): the marks made, drawn where the mask is.
    let cracks_on = options_state.in_force.face_shield_cracks;
    // The weapons' lights and lasers and the head light (`TACTICAL DEVICES`).
    let tactical_on = options_state.in_force.tactical_devices;
    // The shots' bullets (`BALLISTICS`): the calculator, with what it goes by.
    // The effects' entries the hits play, by material.
    let impact_entries = if options_state.in_force.ballistics { trp_data::drawing::particles::material_entries(&mut unity).unwrap_or_default() } else { Vec::new() };
    let impact_names: Vec<String> = impact_entries.iter().map(|e| e.name.clone()).collect();
    let bullets = options_state.in_force.ballistics.then(|| weapons::bullets::Bullets::new(&hard, &db, engine.gravity, impact_entries)).flatten();
    if options_state.in_force.ballistics && bullets.is_none() {
        warn!("the bullets could not be set up: the ballistic presets or the degradation coefficient were not read");
    }
    EffectsAndBullets { waiting, gamma_composed, sound_install, day_install, movement_sounds, world_sounds, hands_sounds, headgear_hearing, grenades, ballistics_on, health_on, visor_on, cracks_on, tactical_on, impact_names, bullets }
}
