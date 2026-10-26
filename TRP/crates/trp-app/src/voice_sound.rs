//! The character's voice and the health's own sounds (health spec, "The
//! voice"; under the options' `HEALTH SYSTEM`): the profile's voice
//! (`Customization.Voice`, `Bear_2_Eng` here) read as the run begins, its
//! hurt, death, agony and breathing banks, and a bone's crack. Each frame
//! the speaker (`trp_sim::speaker`) is told what the health did, what hit
//! the character, whether its breath is heard and whether a face shield
//! muffles it; what it says plays unplaced on the speech source
//! (`Player.PlaySpeechFromTime` in first person: forced stereo, blend 0),
//! a phrase cutting the one before it: the volume the speech's preset
//! gives a source (`Speech`'s `OverallVolume`, 0.7) times the clip's, and
//! the speech's group (`ClientPlayerSpeech`, or `ClientPlayerSelfSpeechReverb`
//! when muffled) with the groups above it, which are under `Main`. A
//! fracture's crack (`Sounds.FractureSound`) plays where the feet are
//! (`BetterAudio.PlayAtPoint`, the `Character` group's rolloff over 15 m).
//! `PROVISIONAL`: the speech group's two equalisers, the muffled group's
//! reverb and the reverb's copy of the source are not built.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use trp_data::audio_preset::AudioGroupPreset;
use trp_sim::health::{Event, Kind};
use trp_sim::speaker::{Bank, BankClip, Said, Situation, Speaker, Status};

use crate::controller::Controller;
use crate::ui_sound::{no_cut, Cut, Group, Levels, Mixer, Samples, Voice};
use crate::{convert, MainCamera};

/// The audio source groups' numbers (`AudioSourceGroupType`).
const CHARACTER: i32 = 3;
const SPEECH: i32 = 6;
/// How far a bone's crack is heard (`PlayAtPoint`'s rolloff, `Player.OnHealthEffectAdded`).
const CRACK_ROLLOFF: f32 = 15.0;

/// What the reading thread brings: the voice's banks with their sounds, the
/// crack, and the two groups' presets.
struct Read {
    voice: trp_data::sound::VoiceBanks,
    crack: Option<trp_data::sound::Clip>,
    speech: Option<AudioGroupPreset>,
    character: Option<AudioGroupPreset>,
}

/// The voice once read, its speaker, what plays, and what the speaker was
/// last told of the character.
#[derive(Resource)]
pub struct VoiceSounds {
    speaker: Speaker,
    /// Each bank's clips: the sound and its volume.
    clips: Vec<Vec<(Option<Samples>, f32)>>,
    crack: Option<Samples>,
    speech: Option<AudioGroupPreset>,
    character: Option<AudioGroupPreset>,
    coming: Option<Mutex<Receiver<Read>>>,
    /// What plays on the speech source, to be cut.
    playing: Option<Cut>,
    /// The parts' points as last seen: a change of a hundredth asks the
    /// breathing again (`OnHealthChanged`).
    parts: [f32; 7],
    audible: bool,
    shield: Option<(bool, bool)>,
    /// `StaminaExhaustionStartsBreathSound`: the legs' exhaustion heard.
    exhaustion_heard: bool,
    logged: u32,
}

/// Starts reading the voice away from the frame, if the options' `HEALTH
/// SYSTEM` is on; off, nothing is read and nothing is said.
pub fn start_reading(app: &mut App, install: trp_data::Install, on: bool) {
    if !on {
        return;
    }
    let exhaustion_heard = app.world().resource::<crate::GameData>().db.config.pointer("/Stamina/StaminaExhaustionStartsBreathSound").and_then(serde_json::Value::as_bool).unwrap_or(true);
    let (send, receive) = channel();
    std::thread::spawn(move || {
        let Some(name) = install.profile_voice() else {
            eprintln!("the profile names no voice; the character says nothing");
            return;
        };
        let voice = match trp_data::sound::voice_banks(&install, &name, &trp_sim::speaker::trigger::READ, true) {
            Ok(voice) => voice,
            Err(e) => {
                eprintln!("the voice {name} could not be read; the character says nothing: {e:#}");
                return;
            }
        };
        let crack = trp_data::sound::fracture_sound(&install).map_err(|e| eprintln!("a bone's crack could not be read; it is silent: {e:#}")).ok();
        let presets = trp_data::audio_preset::audio_group_presets(&install).map_err(|e| eprintln!("the audio groups' presets could not be read; the voice at the clips' own volume: {e:#}")).unwrap_or_default();
        let find = |group: i32| presets.iter().find(|p| p.group == group).cloned();
        let _ = send.send(Read { voice, crack, speech: find(SPEECH), character: find(CHARACTER) });
    });
    app.insert_resource(VoiceSounds {
        speaker: Speaker::new(0x564f_4943),
        clips: Vec::new(),
        crack: None,
        speech: None,
        character: None,
        coming: Some(Mutex::new(receive)),
        playing: None,
        parts: [0.0; 7],
        audible: false,
        shield: None,
        exhaustion_heard,
        logged: 0,
    });
    app.add_systems(Update, speak.run_if(resource_exists::<VoiceSounds>).after(crate::controller::step));
}

/// The frame's voice: the banks taken in when they come; the speaker told
/// what happened, in the game's order (the health's damage and effects,
/// then the hits, the breath, the face shield, its own update), and what
/// it says played.
#[allow(clippy::too_many_arguments)]
pub fn speak(
    mut voice: ResMut<VoiceSounds>,
    mut controller: ResMut<Controller>,
    carried: Res<crate::inventory_view::Carried>,
    game: Res<crate::GameData>,
    mut received: MessageReader<crate::contusion_view::Received>,
    mut again: MessageReader<crate::death::BeginAgain>,
    mixer: Option<Res<Mixer>>,
    levels: Option<Res<Levels>>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    time: Res<Time>,
) {
    let voice = &mut *voice;
    if let Some(coming) = voice.coming.take() {
        match coming.lock().ok().map(|receive| receive.try_recv()) {
            Some(Ok(read)) => {
                let banks = read.voice.banks.iter().map(|b| Bank { trigger: b.trigger, importance: b.importance, ignore_tags: b.ignore_tags, clips: b.clips.iter().map(|c| BankClip { mask: c.mask, length: c.length }).collect() }).collect();
                voice.speaker.set_banks(banks);
                voice.clips = read.voice.banks.into_iter().map(|b| b.clips.into_iter().map(|c| (c.clip.map(Samples::of_owned), c.volume)).collect()).collect();
                info!(
                    "the voice {} ({}): {} banks, {} clips; {}",
                    read.voice.name,
                    read.voice.bundle,
                    voice.clips.len(),
                    voice.clips.iter().flatten().filter(|(c, _)| c.is_some()).count(),
                    if read.crack.is_some() { "a bone's crack" } else { "no bone's crack" }
                );
                voice.crack = read.crack.map(Samples::of_owned);
                (voice.speech, voice.character) = (read.speech, read.character);
            }
            Some(Err(std::sync::mpsc::TryRecvError::Empty)) => voice.coming = Some(coming),
            _ => {}
        }
    }
    let mut heard = std::mem::take(controller.voice_heard.get_or_insert_with(Vec::new));
    // Dead, the controller no longer takes what health did (`controller::step`):
    // the killing hit's damage and the death are taken here.
    if let Some(health) = controller.health.as_mut().filter(|health| !health.alive) {
        heard.extend(health.take_events());
    }
    let hits: Vec<_> = received.read().copied().collect();
    let now = time.elapsed_secs();
    let mut cracks = 0;
    // A run begun again: a new character, its speaker new.
    if again.read().count() > 0 {
        voice.speaker = std::mem::replace(&mut voice.speaker, Speaker::new(0)).renewed();
        if let Some(cut) = voice.playing.take() {
            if let Some(mixer) = mixer.as_deref() {
                cut.store(mixer.0.clock(), std::sync::atomic::Ordering::Release);
            }
        }
        voice.shield = None;
    }
    let Some(health) = controller.health.as_ref() else { return };
    if !voice.speaker.has_banks() {
        return;
    }
    let stamina = &controller.movement.stamina;
    let audible = !stamina.holding_breath() && ((voice.exhaustion_heard && stamina.legs.exhausted()) || stamina.oxygen.exhausted());
    let situation = |health: &trp_sim::health::Health| Situation {
        alive: health.alive,
        status: Status::of(health),
        on_painkillers: controller.movement.conditions.on_painkillers,
        painkiller: health.active(Kind::PainKiller, None).is_some(),
        audible,
    };
    let s = situation(health);
    for event in heard {
        match event {
            Event::Damaged(_, damage, kind) => voice.speaker.damaged(kind, damage, now, &s),
            Event::Started(kind, part) => {
                voice.speaker.effect_started(kind, part);
                if kind == Kind::Fracture {
                    cracks += 1;
                }
            }
            Event::Removed(kind, part, _) => voice.speaker.effect_removed(kind, part),
            Event::Died(kind) => voice.speaker.died(kind, now, &s),
            _ => {}
        }
    }
    for hit in &hits {
        voice.speaker.hit(hit.part, hit.damage, now, &s);
    }
    if s.alive {
        let parts = trp_sim::health::Part::ALL.map(|p| health.part(p).current);
        if parts.iter().zip(voice.parts).any(|(now, was)| (now - was).abs() >= 0.01) {
            voice.parts = parts;
            voice.speaker.breath(now, &s);
        }
        if audible != voice.audible {
            voice.audible = audible;
            voice.speaker.audible_changed(now, &s);
        }
        // The headwear's and the face cover's shields (`FaceShieldObserver`,
        // `FaceCoverObserver`): muffled while either is down over armour on
        // the jaw. PROVISIONAL: the game sets it from the one that changed.
        let equipment = &carried.equipment;
        let shield = ["Headwear", "FaceCover"].map(|slot| (trp_sim::visor::shield_in(equipment, slot).is_some_and(|sh| sh.down), trp_sim::speaker::muffles(&game.db, equipment, slot)));
        let state = (shield.iter().any(|(down, _)| *down), shield.iter().any(|(_, m)| *m));
        if voice.shield != Some(state) {
            let first = voice.shield.is_none();
            voice.shield = Some(state);
            if !first || state.1 {
                voice.speaker.face_shield(state.0, state.1, now, &s);
            }
        }
    }
    voice.speaker.update(now, &s);
    let Some(mixer) = mixer else { return };
    let overall = levels.as_deref().map_or(1.0, Levels::overall_gain);
    for said in voice.speaker.take_said() {
        if let Some(cut) = voice.playing.take() {
            cut.store(mixer.0.clock(), Ordering::Release);
        }
        let Said::Phrase(phrase) = said else { continue };
        let Some((Some(samples), volume)) = voice.clips.get(phrase.bank).and_then(|b| b.get(phrase.clip)) else { continue };
        let base = voice.speech.as_ref().map_or(1.0, |p| p.overall_volume);
        let gain = overall * base * volume * mixer.0.speech_gain(phrase.muffled);
        let cut = no_cut();
        mixer.0.add_to(Group::Main, Voice::whole(samples, gain).at_frame(mixer.0.clock(), false, cut.clone()));
        voice.playing = Some(cut);
        if voice.logged < 64 {
            voice.logged += 1;
            info!("the voice: bank {} clip {}{} at {gain:.3}", phrase.bank, phrase.clip, if phrase.muffled { ", muffled" } else { "" });
        }
    }
    // A bone's crack where the feet are, by the character's group's rolloff.
    if let (Some(crack), Ok(camera)) = (voice.crack.as_ref(), camera.single()) {
        for _ in 0..cracks {
            let at = convert::position(controller.movement.position);
            let to = at - camera.translation();
            let distance = to.length();
            if distance > CRACK_ROLLOFF {
                continue;
            }
            let falloff = voice.character.as_ref().map_or(1.0, |p| p.rolloff_gain(distance, CRACK_ROLLOFF).max(0.0) * p.overall_volume);
            let side = if distance > 1e-4 { (to / distance).dot(camera.right().as_vec3()).clamp(-1.0, 1.0) } else { 0.0 };
            let angle = (side + 1.0) * std::f32::consts::FRAC_PI_4;
            let gain = overall * falloff;
            mixer.0.add_to(Group::Main, Voice::placed(crack, [angle.cos() * gain, angle.sin() * gain], Arc::new(AtomicU32::new(1f32.to_bits()))));
            info!("a bone's crack, {distance:.1} m off");
        }
    }
}
