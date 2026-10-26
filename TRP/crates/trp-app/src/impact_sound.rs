//! A bullet's hit heard (ballistics spec, "What a hit shows and sounds";
//! `Effect.PlaySound`, `BetterAudio.PlayAtPoint`): the bank of the entry its
//! material plays (`Effects.Effect.Sound`), from 5 cm out along the normal,
//! at the hit's volume (the bank's drawn volume times it; none at nought),
//! its clips chosen by the camera's distance from the hit, none past the
//! bank's rolloff. Every impact's bank in the install plays at its point
//! (none is physical or limited). A ricochet off anything but a body plays
//! the effects' first additional bank (`AdditionalSoundEffects[0]`) at the
//! hit's place at full volume. Placed as the spent cases' are: falling as
//! one over the distance past 1 m and panned at equal power
//! (`PROVISIONAL`), in the world's group (`Impacts`:
//! `Main/Environment/CommonSounds`). Read as the run begins with the
//! options' `BALLISTICS` on.

use std::sync::atomic::AtomicU32;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use trp_data::sound::EffectSounds;
use trp_sim::recoil::Rng;
use trp_sim::steps::{choose, Shuffle};

use crate::convert;
use crate::ui_sound::{Levels, Mixer, Samples, Voice};
use crate::MainCamera;

/// How far out along the normal an effect's sound plays (`Effect.Emit`).
const OUT: f32 = 0.05;
/// Where the source's falloff begins (as the cases').
const FULL_WITHIN: f32 = 1.0;

/// The impacts' banks once read, their clips ready to mix, each bank's
/// shuffle and the draws, and what brings them.
#[derive(Resource)]
pub struct ImpactSounds {
    read: Option<EffectSounds>,
    clips: Vec<Option<Samples>>,
    shuffles: Vec<Shuffle>,
    rng: Rng,
    coming: Option<Mutex<Receiver<EffectSounds>>>,
    /// How many have been written to the log.
    logged: usize,
}

/// Starts reading every material entry's sound and the ricochet's, away
/// from the frame.
pub fn start_reading(app: &mut App, install: trp_data::Install, names: Vec<String>) {
    let (send, receive) = channel();
    std::thread::spawn(move || match trp_data::sound::effect_sounds(&install, &names, true) {
        Ok(sounds) => {
            let _ = send.send(sounds);
        }
        Err(e) => eprintln!("the bullets' hits' sounds could not be read; they are silent: {e:#}"),
    });
    app.insert_resource(ImpactSounds { read: None, clips: Vec::new(), shuffles: Vec::new(), rng: Rng::new(0x494d_5041), coming: Some(Mutex::new(receive)), logged: 0 });
}

/// The frame's hits heard.
pub fn play_impacts(bullets: Res<crate::bullets::Bullets>, mut sounds: ResMut<ImpactSounds>, mixer: Option<Res<Mixer>>, levels: Option<Res<Levels>>, camera: Query<&GlobalTransform, With<MainCamera>>) {
    let sounds = &mut *sounds;
    if let Some(coming) = sounds.coming.take() {
        match coming.lock().ok().map(|receive| receive.try_recv()) {
            Some(Ok(mut read)) => {
                sounds.clips = std::mem::take(&mut read.clips).into_iter().map(|(_, clip)| clip.map(Samples::of_owned)).collect();
                sounds.shuffles = vec![Shuffle::default(); read.banks.len()];
                info!("the bullets' hits' sounds: {} entries, {} banks, {} clips read", read.effects.len(), read.banks.len(), sounds.clips.iter().flatten().count());
                sounds.read = Some(read);
            }
            Some(Err(std::sync::mpsc::TryRecvError::Empty)) => sounds.coming = Some(coming),
            _ => {}
        }
    }
    let (Some(mixer), Ok(camera), Some(read)) = (mixer, camera.single(), sounds.read.as_ref()) else { return };
    let overall = levels.map_or(1.0, |levels| levels.overall_gain());
    let mut plays: Vec<(usize, [f32; 3], f32)> = Vec::new();
    // A ricochet's own sound, at the hit's place, at full volume.
    if let Some(at) = read.additional.first().copied().flatten() {
        plays.extend(bullets.ricochets.iter().map(|&place| (at, place, 1.0)));
    }
    // Each effect's bank, 5 cm out, at the hit's volume.
    for (impact, entry) in &bullets.effects {
        if impact.volume <= 0.0 {
            continue;
        }
        let Some(name) = bullets.entries.get(*entry).map(|e| e.name.as_str()) else { continue };
        let Some(at) = read.effects.iter().find(|(n, _)| n == name).and_then(|(_, at)| *at) else { continue };
        let n = impact.normal;
        let place = std::array::from_fn(|k| impact.position[k] + n[k] * OUT);
        plays.push((at, place, impact.volume));
    }
    for (at, place, volume) in plays {
        let bank = &read.banks[at];
        let played = play_at_point(&mixer, bank, &sounds.clips, &mut sounds.shuffles[at], &mut sounds.rng, convert::position(place), camera, volume, overall, None);
        if let (Some((distance, drawn)), true) = (played, sounds.logged < 32) {
            sounds.logged += 1;
            info!("a hit's sound ({}) {distance:.1} m off at {:.3}", bank.name, volume * drawn);
        }
    }
}

/// A bank played at a point in the world's group, as the game's
/// `BetterAudio.PlayAtPoint` plays one: nothing past the bank's rolloff;
/// its clips chosen by the camera's distance (`SoundBank.PickClips`); the
/// bank's volume drawn (less up to its `DeltaVolume`) times the volume
/// asked, clamped to 1; falling with the distance by its source group's
/// rolloff curve where it is given (`falloff`: the group's preset, its
/// `SoundRolloff` over the bank's rolloff as the source's farthest, times
/// its `OverallVolume`), else as one over the distance past 1 m
/// (`PROVISIONAL`, the falloff the app's other sounds of the world still
/// take); panned at equal power by how far to the camera's side it lies
/// (`PROVISIONAL`: the binaural plug-in is not built). The distance and
/// the drawn volume, if it played.
#[allow(clippy::too_many_arguments)]
pub(crate) fn play_at_point(
    mixer: &Mixer,
    bank: &trp_data::sound::Bank,
    clips: &[Option<Samples>],
    shuffle: &mut Shuffle,
    rng: &mut Rng,
    source: Vec3,
    camera: &GlobalTransform,
    volume: f32,
    overall: f32,
    falloff: Option<&trp_data::audio_preset::AudioGroupPreset>,
) -> Option<(f32, f32)> {
    let to_source = source - camera.translation();
    let distance = to_source.length();
    if distance > bank.rolloff {
        return None;
    }
    let choice = choose(bank, shuffle, distance, rng);
    let drawn = if bank.delta_volume > 0.0 { bank.base_volume + rng.range(-bank.delta_volume, 0.0) } else { bank.base_volume };
    let falloff = match falloff {
        Some(preset) => preset.rolloff_gain(distance, bank.rolloff).max(0.0) * preset.overall_volume,
        None if distance > FULL_WITHIN => FULL_WITHIN / distance,
        None => 1.0,
    };
    let side = if distance > 1e-4 { (to_source / distance).dot(camera.right().as_vec3()).clamp(-1.0, 1.0) } else { 0.0 };
    let angle = (side + 1.0) * std::f32::consts::FRAC_PI_4;
    let gain_all = (volume * drawn).clamp(0.0, 1.0) * overall * falloff;
    let pitch = Arc::new(AtomicU32::new(choice.pitch.to_bits()));
    let both = choice.clips.iter().all(Option::is_some);
    for (k, clip) in choice.clips.iter().enumerate() {
        let Some(samples) = clip.and_then(|clip| clips.get(clip)?.as_ref()) else { continue };
        let share = if !both {
            1.0
        } else if k == 0 {
            choice.share
        } else {
            1.0 - choice.share
        };
        let gain = gain_all * share;
        mixer.0.add_to(crate::ui_sound::Group::Main, Voice::placed(samples, [angle.cos() * gain, angle.sin() * gain], pitch.clone()));
    }
    Some((distance, drawn))
}
