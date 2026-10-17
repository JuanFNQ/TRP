//! The sounds of the player's own moving (`docs/spec/footsteps.md`; the
//! user, 2026-10-16: "Lets add the sounds for moving in the world"): the
//! install's banks for every surface and the kit (`trp_data::sound::
//! movement_sounds`), read away from the frame as the run begins, and the
//! sounds the simulation's rules ask for (`trp_sim::steps`) played into the
//! app's one source (`ui_sound::Mixer`) as the game plays them in first
//! person:
//!
//! - each from its bank's choice of clips, shares and pitch
//!   (`trp_sim::steps::choose`), the pitch set on the source it plays from,
//!   the steps' or the kit's, which a sound still playing follows
//!   (`PROVISIONAL`);
//! - its volume clamped to 0 to 1 (`SoundBank.Play`), the kit's source at
//!   0.85 (`FP_GEAR_VOLUME`), at the overall volume and the mixer's
//!   `ClientPlayerMovement` group's (-3.5 dB as read with the groups above
//!   it, `ui_sound::GameMix`; its two equalisers not built);
//! - from a point 0.1 m over the feet, heard at the camera: its level falls
//!   as one over the distance past 1 m (the source's logarithmic rolloff
//!   from 1 m), and it is panned by how far to the side of the camera it
//!   lies, at equal power (`PROVISIONAL`: the engine's pan law is not
//!   read).
//!
//! With the options' `MOVEMENT SOUNDS` off nothing is read and nothing
//! plays; the rules still run, as the bob's timing is theirs too.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use trp_data::sound::MovementSounds;
use trp_sim::recoil::Rng;
use trp_sim::steps::{choose, BankOf, Figures, Shuffle};

use crate::controller::Controller;
use crate::ui_sound::{Levels, Mixer, Pitch, Samples, Voice};
use crate::MainCamera;

/// The banks once read, each bank's shuffle, the clips ready to mix, the
/// two sources' pitches, and what brings them.
#[derive(Resource)]
pub struct StepBanks {
    sounds: Option<MovementSounds>,
    clips: Vec<Option<Samples>>,
    names: Vec<String>,
    shuffles: Vec<Shuffle>,
    figures: Vec<(u8, [Option<Figures>; 9])>,
    rng: Rng,
    steps: Pitch,
    kit: Pitch,
    coming: Option<Mutex<Receiver<MovementSounds>>>,
}

impl Default for StepBanks {
    fn default() -> Self {
        Self {
            sounds: None,
            clips: Vec::new(),
            names: Vec::new(),
            shuffles: Vec::new(),
            figures: Vec::new(),
            rng: Rng::new(0x5354_4550),
            steps: Arc::new(AtomicU32::new(1.0f32.to_bits())),
            kit: Arc::new(AtomicU32::new(1.0f32.to_bits())),
            coming: None,
        }
    }
}

/// The kit's source's level in first person (`Player.FP_GEAR_VOLUME`).
const KIT_VOLUME: f32 = 0.85;
/// The sources stand this far over the feet (`LocalPosition`).
const OVER_FEET: f32 = 0.1;
/// Where the source's logarithmic falloff begins (`MinDistance` of the
/// prefab `AudioSourceSpatialBuiltinEQReverbPrefab`).
const FULL_WITHIN: f32 = 1.0;

/// Starts reading the banks away from the frame, if the options' `MOVEMENT
/// SOUNDS` is on.
pub fn start_reading(app: &mut App, install: trp_data::Install, on: bool) {
    let mut banks = StepBanks::default();
    if on {
        let (send, receive) = channel();
        std::thread::spawn(move || match trp_data::sound::movement_sounds(&install, None, true) {
            Ok(sounds) => {
                let _ = send.send(sounds);
            }
            Err(e) => eprintln!("the sounds of moving could not be read; they are silent: {e:#}"),
        });
        banks.coming = Some(Mutex::new(receive));
    }
    app.insert_resource(banks);
}

/// Takes the banks in once they have come: the clips made ready to mix and
/// the banks' figures worked out for the rules.
fn take_banks(mut banks: ResMut<StepBanks>) {
    let Some(coming) = banks.coming.take() else { return };
    let got = coming.lock().ok().and_then(|receive| match receive.try_recv() {
        Ok(sounds) => Some(Some(sounds)),
        Err(std::sync::mpsc::TryRecvError::Empty) => None,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(None),
    });
    match got {
        None => banks.coming = Some(coming),
        Some(None) => {}
        Some(Some(mut sounds)) => {
            let (names, clips): (Vec<String>, Vec<Option<Samples>>) = std::mem::take(&mut sounds.clips).into_iter().map(|(name, clip)| (name, clip.map(Samples::of_owned))).unzip();
            let figures = sounds
                .surfaces
                .iter()
                .map(|set| (set.surface, std::array::from_fn(|way| set.banks.get(way).copied().flatten().map(|at| Figures::of(&sounds.banks[at])))))
                .collect();
            info!("the sounds of moving: {} banks of {} surfaces, {} clips read", sounds.banks.len(), sounds.surfaces.len(), clips.iter().flatten().count());
            banks.shuffles = vec![Shuffle::default(); sounds.banks.len()];
            banks.clips = clips;
            banks.names = names;
            banks.figures = figures;
            banks.sounds = Some(sounds);
        }
    }
}

/// A line for every sound of moving played, to a file (`--log-sounds`):
/// the run's time, the surface, the bank, the clips with their shares,
/// the pitch, the volume asked and the two channels' gains.
#[derive(Resource)]
pub struct SoundLog(pub Mutex<std::io::BufWriter<std::fs::File>>);

/// Plays what the rules asked for on the frame's steps.
fn play(
    mut controller: ResMut<Controller>,
    mut banks: ResMut<StepBanks>,
    mixer: Option<Res<Mixer>>,
    levels: Option<Res<Levels>>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    log: Option<Res<SoundLog>>,
    time: Res<Time>,
) {
    let sounds = &mut controller.movement.sounds;
    if !banks.figures.is_empty() && !sounds.has_figures() {
        sounds.set_figures(banks.figures.clone());
    }
    let asked = sounds.take();
    let (Some(mixer), Ok(camera)) = (mixer, camera.single()) else { return };
    if asked.is_empty() || banks.sounds.is_none() {
        return;
    }
    let overall = levels.map_or(1.0, |levels| levels.overall_gain()) * mixer.0.movement_gain();
    // From the point over the feet to the camera: the falloff and the pan.
    let p = controller.movement.position;
    let source = Vec3::new(p[0], p[1] + OVER_FEET, -p[2]);
    let to_source = source - camera.translation();
    let distance = to_source.length();
    let falloff = if distance > FULL_WITHIN { FULL_WITHIN / distance } else { 1.0 };
    let side = if distance > 1e-4 { (to_source / distance).dot(camera.right().as_vec3()).clamp(-1.0, 1.0) } else { 0.0 };
    let angle = (side + 1.0) * std::f32::consts::FRAC_PI_4;
    let pan = [angle.cos(), angle.sin()];
    let banks = &mut *banks;
    let Some(sounds) = banks.sounds.as_ref() else { return };
    for play in asked {
        let at = match play.bank {
            BankOf::Surface(surface, way) => sounds.surface(surface).and_then(|set| set.banks.get(way).copied().flatten()),
            BankOf::Kit(which) => sounds.gear.get(which).copied().flatten(),
        };
        let Some(at) = at else { continue };
        let choice = choose(&sounds.banks[at], &mut banks.shuffles[at], play.blend, &mut banks.rng);
        let pitch = if play.kit { &banks.kit } else { &banks.steps };
        pitch.store(choice.pitch.to_bits(), Ordering::Relaxed);
        let volume = play.volume.clamp(0.0, 1.0) * if play.kit { KIT_VOLUME } else { 1.0 } * overall * falloff;
        let both = choice.clips.iter().all(Option::is_some);
        let mut said = Vec::new();
        for (n, clip) in choice.clips.iter().enumerate() {
            let Some((place, samples)) = clip.and_then(|clip| Some((clip, banks.clips.get(clip)?.as_ref()?))) else { continue };
            let share = if !both {
                1.0
            } else if n == 0 {
                choice.share
            } else {
                1.0 - choice.share
            };
            let gain = volume * share;
            mixer.0.add(Voice::placed(samples, [pan[0] * gain, pan[1] * gain], pitch.clone()));
            said.push(format!("{} x{share:.3}", banks.names[place]));
        }
        if let Some(log) = &log {
            use std::io::Write;
            let surface = match play.bank {
                BankOf::Surface(surface, _) => trp_data::sound::SURFACES.get(surface as usize).copied().unwrap_or("?"),
                BankOf::Kit(_) => "kit",
            };
            if let Ok(mut out) = log.0.lock() {
                let _ = writeln!(
                    out,
                    "{:9.3} {surface:12} {:28} blend {:.3} asked {:.4} volume {:.4} pitch {:.3} gains {:.4} {:.4} distance {distance:.3} [{}]",
                    time.elapsed_secs(),
                    sounds.banks[at].name,
                    play.blend,
                    play.volume,
                    volume,
                    choice.pitch,
                    pan[0] * volume,
                    pan[1] * volume,
                    said.join(", ")
                );
                let _ = out.flush();
            }
        }
    }
}

pub struct StepSoundPlugin;

impl Plugin for StepSoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (take_banks, play).chain().after(crate::controller::step));
    }
}
