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
use trp_data::audio_preset::AudioGroupPreset;
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
    /// The switches on the head's clips, by `TOGGLE_SOUNDS`' order.
    toggles: Vec<Option<Samples>>,
    shuffles: Vec<Shuffle>,
    figures: Vec<(u8, [Option<Figures>; 9])>,
    rng: Rng,
    steps: Pitch,
    kit: Pitch,
    /// The `Character` group's preset, whose rolloff a character seen from
    /// outside is heard by, and each dummy's two sources' pitches (steps,
    /// kit), by its number.
    character: Option<AudioGroupPreset>,
    dummy_pitches: Vec<[Pitch; 2]>,
    coming: Option<Mutex<Receiver<(MovementSounds, Option<AudioGroupPreset>)>>>,
}

impl Default for StepBanks {
    fn default() -> Self {
        Self {
            sounds: None,
            clips: Vec::new(),
            names: Vec::new(),
            toggles: Vec::new(),
            shuffles: Vec::new(),
            figures: Vec::new(),
            rng: Rng::new(0x5354_4550),
            steps: Arc::new(AtomicU32::new(1.0f32.to_bits())),
            kit: Arc::new(AtomicU32::new(1.0f32.to_bits())),
            character: None,
            dummy_pitches: Vec::new(),
            coming: None,
        }
    }
}

/// The source group a character's sounds of moving play from
/// (`AudioSourceGroupType.Character`).
const CHARACTER: i32 = 3;

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
                let character = match trp_data::audio_preset::audio_group_presets(&install) {
                    Ok(presets) => presets.into_iter().find(|preset| preset.group == CHARACTER),
                    Err(e) => {
                        eprintln!("the audio groups' presets could not be read; a dummy's steps fall off as the app's other sounds do: {e:#}");
                        None
                    }
                };
                let _ = send.send((sounds, character));
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
        Some(Some((mut sounds, character))) => {
            banks.character = character;
            let (names, clips): (Vec<String>, Vec<Option<Samples>>) = std::mem::take(&mut sounds.clips).into_iter().map(|(name, clip)| (name, clip.map(Samples::of_owned))).unzip();
            let figures = sounds
                .surfaces
                .iter()
                .map(|set| (set.surface, std::array::from_fn(|way| set.banks.get(way).copied().flatten().map(|at| Figures::of(&sounds.banks[at])))))
                .collect();
            info!("the sounds of moving: {} banks of {} surfaces, {} clips read", sounds.banks.len(), sounds.surfaces.len(), clips.iter().flatten().count());
            banks.toggles = trp_data::sound::TOGGLE_SOUNDS
                .iter()
                .map(|name| sounds.toggles.iter_mut().find(|(asked, ..)| asked == name).and_then(|(.., clip)| clip.take()).map(Samples::of_owned))
                .collect();
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
        // And the kit's banks' (a mount's medium rattle draws its volume).
        if let Some(read) = banks.sounds.as_ref() {
            sounds.set_kit_figures(std::array::from_fn(|which| read.gear.get(which).copied().flatten().map(|at| Figures::of(&read.banks[at]))));
        }
    }
    let asked = sounds.take();
    // A switch on the head's sound: the character's own, not placed in the
    // world, at full volume through the movement's group
    // (`ClientPlayer.PlayToggleSound`).
    let toggled = std::mem::take(&mut controller.toggle_sounds);
    if let Some(mixer) = mixer.as_deref() {
        let gain = levels.as_deref().map_or(1.0, |levels| levels.overall_gain()) * mixer.0.movement_gain();
        for clip in toggled.into_iter().filter_map(|at| banks.toggles.get(at)?.as_ref()) {
            mixer.0.add_to(crate::ui_sound::Group::Main, Voice::whole(clip, gain));
        }
    }
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
        // A vault's play names its element or its kit bank (footsteps
        // spec, "The vault's sounds"): the element's bank at the height's
        // place in its volume range, the kit's at the height's share of
        // its own; either times the bank's base volume and its drawn
        // volume, as a bank played at a point is (`PlayAtPoint`), from a
        // source of its own with its own drawn pitch.
        let vault = matches!(play.bank, BankOf::Vault { .. } | BankOf::VaultGear(_));
        let (at, volume) = match play.bank {
            BankOf::Surface(surface, way) => (sounds.surface(surface).and_then(|set| set.banks.get(way).copied().flatten()), play.volume),
            BankOf::Kit(which) => (sounds.gear.get(which).copied().flatten(), play.volume),
            BankOf::Vault { kind, mark, surface } => match sounds.vault_element(surface, kind, mark) {
                Some(element) => (element.bank, element.min_volume + (element.max_volume - element.min_volume) * play.volume.clamp(0.0, 1.0)),
                None => (None, 0.0),
            },
            BankOf::VaultGear(kind) => (sounds.vault_gear.get(kind).copied().flatten(), play.volume),
        };
        let Some(at) = at else { continue };
        let volume = if vault {
            let bank = &sounds.banks[at];
            let drawn = if bank.delta_volume > 0.0 { bank.base_volume + banks.rng.range(-bank.delta_volume, 0.0) } else { bank.base_volume };
            volume * bank.base_volume * drawn
        } else {
            volume
        };
        let choice = choose(&sounds.banks[at], &mut banks.shuffles[at], play.blend, &mut banks.rng);
        let pitch: Pitch = if vault {
            Arc::new(AtomicU32::new(choice.pitch.to_bits()))
        } else {
            let cell = if play.kit { &banks.kit } else { &banks.steps };
            cell.store(choice.pitch.to_bits(), Ordering::Relaxed);
            cell.clone()
        };
        let volume = volume.clamp(0.0, 1.0) * if play.kit { KIT_VOLUME } else { 1.0 } * overall * falloff;
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
            mixer.0.add_to(crate::ui_sound::Group::Main, Voice::placed(samples, [pan[0] * gain, pan[1] * gain], pitch.clone()));
            said.push(format!("{} x{share:.3}", banks.names[place]));
        }
        if let Some(log) = &log {
            use std::io::Write;
            let surface = match play.bank {
                BankOf::Surface(surface, _) | BankOf::Vault { surface, .. } => trp_data::sound::SURFACES.get(surface as usize).copied().unwrap_or("?"),
                BankOf::Kit(_) | BankOf::VaultGear(_) => "kit",
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

/// The dummies' sounds of moving, heard from outside (footsteps spec, "A
/// character seen from outside"): each living dummy's rules run as the
/// player's do, seen from outside (`StepSounds::third_person`), and what
/// they ask for is taken every frame and played as the game plays another
/// character's, from its two sources (its steps', its kit's) 0.1 m over its
/// feet:
///
/// - each sound's source heard no farther than the movement's base rolloff
///   for a character seen from outside (`BaseMaxMovementRolloff`, 90 m) times
///   the kind's multiplier its rolloff was set by, times the outdoors' share
///   (`OutdoorRolloffMult`, `PROVISIONAL`: the app's range has no indoors);
///   past it, nothing (`SoundBank.Play`);
/// - every sound but a footstep chooses its bank's list by the distance;
/// - its volume clamped to 0 to 1, the kit's source at 1
///   (`LOCAL_AI_GEAR_VOLUME`), falling by the `Character` group's rolloff
///   curve over the source's farthest (its `SoundRolloff`, from the
///   install's preset) at its `OverallVolume`; through the mixer's
///   `ObservedPlayerMovement` group, at 0 dB with the groups above it (not
///   the player's own -3.5 dB); panned at equal power (`PROVISIONAL`: the
///   game's binaural plug-in and its occlusion are not built);
/// - the pitch of each source its own, a dummy's own two.
///
/// With `MOVEMENT SOUNDS` off nothing plays; the asks are taken all the
/// same.
#[allow(clippy::too_many_arguments)]
fn play_dummies(
    dummies: Option<ResMut<crate::dummy::Dummies>>,
    mut banks: ResMut<StepBanks>,
    mixer: Option<Res<Mixer>>,
    levels: Option<Res<Levels>>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    log: Option<Res<SoundLog>>,
    time: Res<Time>,
) {
    let Some(mut dummies) = dummies else { return };
    let banks = &mut *banks;
    let overall = levels.map_or(1.0, |levels| levels.overall_gain());
    let camera = camera.single().ok();
    for (which, feet, rules) in dummies.movers() {
        if !banks.figures.is_empty() && !rules.has_figures() {
            rules.set_figures(banks.figures.clone());
        }
        let asked = rules.take();
        let (Some(mixer), Some(camera), Some(sounds)) = (mixer.as_deref(), camera, banks.sounds.as_ref()) else { continue };
        if asked.is_empty() {
            continue;
        }
        while banks.dummy_pitches.len() <= which {
            banks.dummy_pitches.push(std::array::from_fn(|_| Arc::new(AtomicU32::new(1.0f32.to_bits()))));
        }
        let source = Vec3::new(feet[0], feet[1] + OVER_FEET, -feet[2]);
        let to_source = source - camera.translation();
        let distance = to_source.length();
        let side = if distance > 1e-4 { (to_source / distance).dot(camera.right().as_vec3()).clamp(-1.0, 1.0) } else { 0.0 };
        let angle = (side + 1.0) * std::f32::consts::FRAC_PI_4;
        let settings = &rules.settings;
        for play in asked {
            // A vault's or a climb's: the dummies make neither.
            let at = match play.bank {
                BankOf::Surface(surface, way) => sounds.surface(surface).and_then(|set| set.banks.get(way).copied().flatten()),
                BankOf::Kit(which) => sounds.gear.get(which).copied().flatten(),
                BankOf::Vault { .. } | BankOf::VaultGear(_) => None,
            };
            let Some(at) = at else { continue };
            let farthest = settings.third_person_rolloff * play.reach.unwrap_or(1.0) * settings.outdoor_rolloff;
            if distance > farthest {
                continue;
            }
            let blend = if play.by_distance { distance } else { play.blend };
            let choice = choose(&sounds.banks[at], &mut banks.shuffles[at], blend, &mut banks.rng);
            let pitch = banks.dummy_pitches[which][usize::from(play.kit)].clone();
            pitch.store(choice.pitch.to_bits(), Ordering::Relaxed);
            let falloff = match banks.character.as_ref() {
                Some(preset) => preset.rolloff_gain(distance, farthest).max(0.0) * preset.overall_volume,
                None if distance > FULL_WITHIN => FULL_WITHIN / distance,
                None => 1.0,
            };
            let volume = play.volume.clamp(0.0, 1.0) * overall * falloff;
            let both = choice.clips.iter().all(Option::is_some);
            for (n, clip) in choice.clips.iter().enumerate() {
                let Some(samples) = clip.and_then(|clip| banks.clips.get(clip)?.as_ref()) else { continue };
                let share = if !both {
                    1.0
                } else if n == 0 {
                    choice.share
                } else {
                    1.0 - choice.share
                };
                let gain = volume * share;
                mixer.0.add_to(crate::ui_sound::Group::Main, Voice::placed(samples, [angle.cos() * gain, angle.sin() * gain], pitch.clone()));
            }
            if let Some(Ok(mut out)) = log.as_ref().map(|log| log.0.lock()) {
                use std::io::Write;
                let _ = writeln!(
                    out,
                    "{:9.3} dummy {which} {:28} blend {blend:.3} asked {:.4} farthest {farthest:.1} falloff {falloff:.4} volume {volume:.4} pitch {:.3} distance {distance:.3}",
                    time.elapsed_secs(),
                    sounds.banks[at].name,
                    play.volume,
                    choice.pitch
                );
                let _ = out.flush();
            }
        }
    }
}

pub struct StepSoundPlugin;

impl Plugin for StepSoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (take_banks, play, play_dummies).chain().after(crate::controller::step));
    }
}
