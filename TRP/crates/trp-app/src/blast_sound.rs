//! A grenade's blast's sound (grenade spec, "The blast's sound";
//! `Effects.EmitGrenade`, `Effect.PlaySound`, `BetterAudio.PlayAtPointDistant`,
//! `SuperSourceDistant`): the bank of the grenade's effect
//! (`ExplosionEffectType`) played for the blast's place 5 cm up, its clips
//! chosen by the camera's distance from the blast, none past its rolloff.
//! A bank the effect plays from afar (`Physical`, as every frag's) is
//! heard after the sound's travel (the distance over 340 m/s) with a
//! tenth of it unplaced (its spatial blend 0.9), any other at once and
//! wholly placed (`PROVISIONAL`); placed, it falls as one over the
//! distance past 1 m (the distant source's logarithmic rolloff from 1 m)
//! and is panned at equal power, as the steps' (`PROVISIONAL`: the pan
//! law). Through its group's mixer path (`Grenades`: the gunshots').
//! Read as the run begins with the options' `GRENADES AND KNIFE` on.

use std::sync::atomic::AtomicU32;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use trp_data::sound::EffectSounds;
use trp_sim::recoil::Rng;
use trp_sim::steps::{choose, Shuffle};

use crate::convert;
use crate::loose::Blasted;
use crate::ui_sound::{no_cut, Levels, Mixer, Samples, Voice, MIDDLE, MIX_RATE};
use crate::MainCamera;

/// How far over the blast its sound plays (`Effect.Emit`: along the
/// normal, up, by 0.05).
const OVER: f32 = 0.05;
/// The speed of the sound's travel (`PlayAtPointDistant`: its delay is the
/// distance over 340).
const SOUND_SPEED: f32 = 340.0;
/// The spatial blend a sound from afar plays with (`Effect.PlaySound`'s
/// `Mathf.Clamp(0.2, 0.9, distance / 30)`: 0.2 taken up to its least, 0.9,
/// whatever the distance).
const DISTANT_BLEND: f32 = 0.9;
/// Where the source's falloff begins (`AudioSourceSuperSpatialBuiltinEQPrefab`'s
/// `MinDistance`).
const FULL_WITHIN: f32 = 1.0;

/// The blasts' banks once read, their clips ready to mix, each bank's
/// shuffle and the draws, and what brings them.
#[derive(Resource)]
pub struct BlastSounds {
    read: Option<EffectSounds>,
    clips: Vec<Option<Samples>>,
    shuffles: Vec<Shuffle>,
    rng: Rng,
    coming: Option<Mutex<Receiver<EffectSounds>>>,
}

/// Starts reading every grenade's blast's sound away from the frame.
pub fn start_reading(app: &mut App, install: trp_data::Install) {
    let db = &app.world().resource::<crate::GameData>().db;
    let mut names: Vec<String> = db.items.values().filter_map(|item| item.props.get("ExplosionEffectType")?.as_str().map(str::to_owned)).filter(|name| !name.is_empty()).collect();
    names.sort();
    names.dedup();
    let (send, receive) = channel();
    std::thread::spawn(move || match trp_data::sound::effect_sounds(&install, &names, true) {
        Ok(sounds) => {
            let _ = send.send(sounds);
        }
        Err(e) => eprintln!("the grenades' blasts' sounds could not be read; they are silent: {e:#}"),
    });
    app.insert_resource(BlastSounds { read: None, clips: Vec::new(), shuffles: Vec::new(), rng: Rng::new(0x424c_4153), coming: Some(Mutex::new(receive)) });
}

/// Each blast of the frame heard (`loose::Blasted`).
pub fn play_blasts(
    mut blasts: MessageReader<Blasted>,
    mut sounds: ResMut<BlastSounds>,
    game: Res<crate::GameData>,
    mixer: Option<Res<Mixer>>,
    levels: Option<Res<Levels>>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
) {
    let sounds = &mut *sounds;
    if let Some(coming) = sounds.coming.take() {
        match coming.lock().ok().map(|receive| receive.try_recv()) {
            Some(Ok(mut read)) => {
                sounds.clips = std::mem::take(&mut read.clips).into_iter().map(|(_, clip)| clip.map(Samples::of_owned)).collect();
                sounds.shuffles = vec![Shuffle::default(); read.banks.len()];
                info!("the grenades' blasts' sounds: {} effects, {} banks, {} clips read", read.effects.len(), read.banks.len(), sounds.clips.iter().flatten().count());
                sounds.read = Some(read);
            }
            Some(Err(std::sync::mpsc::TryRecvError::Empty)) => sounds.coming = Some(coming),
            _ => {}
        }
    }
    let (Some(mixer), Ok(camera)) = (mixer, camera.single()) else {
        blasts.clear();
        return;
    };
    let overall = levels.map_or(1.0, |levels| levels.overall_gain());
    for blasted in blasts.read() {
        let Some(read) = sounds.read.as_ref() else { continue };
        let Some(effect) = game.db.items.get(&blasted.template).and_then(|item| item.props.get("ExplosionEffectType")).and_then(|name| name.as_str()) else { continue };
        let Some(at) = read.effects.iter().find(|(name, _)| name == effect).and_then(|(_, at)| *at) else { continue };
        let bank = &read.banks[at];
        let blast = convert::position(blasted.at);
        let distance = (blast - camera.translation()).length();
        if distance > bank.rolloff {
            continue;
        }
        let choice = choose(bank, &mut sounds.shuffles[at], distance, &mut sounds.rng);
        let drawn = if bank.delta_volume > 0.0 { bank.base_volume + sounds.rng.range(-bank.delta_volume, 0.0) } else { bank.base_volume };
        // The group's path: the gunshots' for the gunshots' and the
        // grenades' sources (`Guns/Gunshots`), else 0 dB as read.
        let group = if matches!(bank.source_type, 0 | 11) { mixer.0.gunshots_gain() } else { 1.0 };
        let volume = drawn.clamp(0.0, 1.0) * overall * group;
        let (blend, delay) = if bank.physical { (DISTANT_BLEND, distance / SOUND_SPEED) } else { (1.0, 0.0) };
        let to_source = blast + Vec3::Y * OVER - camera.translation();
        let heard = to_source.length();
        let falloff = if heard > FULL_WITHIN { FULL_WITHIN / heard } else { 1.0 };
        let side = if heard > 1e-4 { (to_source / heard).dot(camera.right().as_vec3()).clamp(-1.0, 1.0) } else { 0.0 };
        let angle = (side + 1.0) * std::f32::consts::FRAC_PI_4;
        let gains = [angle.cos(), angle.sin()].map(|pan| (1.0 - blend) * MIDDLE + blend * pan * falloff);
        let pitch = Arc::new(AtomicU32::new(choice.pitch.to_bits()));
        let starts = mixer.0.clock() + (delay * MIX_RATE as f32) as u64;
        let both = choice.clips.iter().all(Option::is_some);
        for (n, clip) in choice.clips.iter().enumerate() {
            let Some(samples) = clip.and_then(|clip| sounds.clips.get(clip)?.as_ref()) else { continue };
            let share = if !both {
                1.0
            } else if n == 0 {
                choice.share
            } else {
                1.0 - choice.share
            };
            let gain = volume * share;
            mixer.0.add_to(crate::ui_sound::Group::Guns, Voice::placed(samples, gains.map(|g| g * gain), pitch.clone()).at_frame(starts, false, no_cut()));
        }
        info!("a blast's sound ({effect}, {}) {distance:.1} m off, heard {delay:.3} s later at {volume:.3}", bank.name);
    }
}
