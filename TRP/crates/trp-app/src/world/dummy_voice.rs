//! The dummies' voices.
//! After the health spec, "The voice", "Any character's voice"; under the options' `HEALTH SYSTEM`
//! with `TARGETS`: each living dummy speaks as the game's `Player` class makes any character speak,
//! a bot included, with a speaker of its own (`trp_sim::speaker::Speaker::of_ai`): the hurt's grunt
//! on its damage (`OnAudioHealthApplyDamage`; a hit has no grunt of its own for a bot,
//! `ApplyHitDebuff`), its hurt bank's chain after it (`BaseSpeaker.Play`, `Queue`), the death's or
//! the agony's cry unless the head was hit last (`OnDead`), the breathing (`UpdateBreathStatus`:
//! hurt badly, out of breath, or muffled), and a bone's crack at its feet (`OnHealthEffectAdded`).
//! What a bot's mind says (contact, mutter, enemy down) is its AI's and is not here.
//!
//! **Its voice** is drawn as SPT's server draws a PMC bot's
//! (`BotGenerator.GenerateBot`: `appearance.voice` of its side's type,
//! `bots/types/usec.json` and `bear.json`, a weighted pick), anew each run
//! (`main`'s `dummy_voices`); its phrases are found by the voice
//! customization's own name (`Customizations::voice_name`). Each voice's
//! four health banks and the banks they chain to are read on a thread of
//! their own as the run begins, sounds and all.
//!
//! **Where it is heard** (`Player.CreateSpeechSource`, `PlaySpeechFromTime`
//! seen from outside): from its head (`StartTrackingPosition` of the head
//! bone), the clip's falloff times the outdoors' share
//! (`SetRolloff`: `OutdoorRolloffMult`; `ProtagonistHearing` is 1 for a
//! `Player`) as the source's farthest, not played at all past it and a
//! metre (`IsOutOfProtagonistsRange`, `CHECK_RANGE_BUFF`); the volume the
//! speech's preset gives a source (`Speech`'s `OverallVolume`, 0.7) times
//! the clip's, falling by that preset's curve over the farthest; in
//! `ObservedPlayer/ObservedPlayerSpeech` (-5 dB), or muffled by its face
//! shield over armour on the jaw in `Occlusion/SimpleOccluded` (0 dB,
//! `UpdateMuffledState`).
//!
//! **It goes with the head** (`BetterSource.ManualUpdate`: the source put
//! where the head is on every frame, the engine working out a placed
//! source's volume and side as it plays): on every frame a phrase plays,
//! what each channel takes of it is worked out anew from where the head is
//! then and where the listener is and faces (`speech_gains`), and the mix
//! is told (`ui_sound::Gains`: it glides to them), until the clip is over
//! or cut. The reach is asked as a phrase begins and not after: one begun
//! in reach falls to nought past its farthest and is not cut. The head is
//! asked on a frame something is said or a phrase plays, not on the others
//! (asking makes its pose).
//!
//! `PROVISIONAL`, as for the app's other placed sounds: panned at equal
//! power (the game's binaural plug-in, its directivity and occlusion are
//! not built), the two groups' filters not applied, the range taken as
//! outdoors. `PROVISIONAL` besides: every phrase of a frame drawn with the
//! health as the frame left it (the game's as each damage left it), either
//! face shield down over jaw armour muffling (the game sets it from the
//! one that changed), as the player's, and a phrase kept in the group it
//! began in (the game sets the source's group at the character's next
//! sound after its shield changed).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy::transform::helper::TransformHelper;
use trp_data::audio::audio_preset::AudioGroupPreset;
use trp_sim::condition::health::{Event, Kind, Part};
use trp_sim::sound::speaker::{trigger, Bank, Said, Situation, Speaker, Status};

use crate::world::dummy::{Dummies, DummyRigs, Heard};
use crate::audio::ui_sound::{gains_of, no_cut, pan, set_gains, Cut, Gains, Group, Levels, Mixer, Samples, Voice};
use crate::audio::voice_sound::{CHARACTER, CRACK_ROLLOFF, SPEECH};
use crate::MainCamera;

/// The banks the health says (`OnAgony`, `OnBeingHurt`, `OnDeath`,
/// `OnBreath`); a bot's berserk would say its fight's, which a dummy never
/// has (a berserk needs Stress Resistance's elite level: its skills are at
/// level 1).
const HEALTH: [i32; 4] = [trigger::AGONY, trigger::BEING_HURT, trigger::DEATH, trigger::BREATH];

/// How far past its farthest the listener may be for a phrase to be
/// played (`Player.CHECK_RANGE_BUFF`).
const CHECK_RANGE_BUFF: f32 = 1.0;

/// The bone the speech source follows.
const HEAD: &str = "Base HumanHead";

/// What the reading thread brings.
enum Read {
    /// The speech's and the character's presets, and a bone's crack.
    Common { speech: Option<AudioGroupPreset>, character: Option<AudioGroupPreset>, crack: Option<trp_data::audio::sound::Clip> },
    /// A dummy's voice, by its number, or why it could not be read.
    Voice(usize, Result<trp_data::audio::sound::VoiceBanks, String>),
}

/// A phrase a dummy's speech source plays: where the mix cuts it, the gains
/// the mix is told anew each frame, the app's second its clip is over at,
/// and what its gains go by besides where it is: its clip's volume, its
/// farthest, and whether it is muffled.
struct Playing {
    cut: Cut,
    gains: Gains,
    until: f32,
    volume: f32,
    farthest: f32,
    muffled: bool,
    /// The gains the sounds' log last had of it.
    logged: [f32; 2],
}

impl Playing {
    /// Cut at the mix's frame now.
    fn cut(self, mixer: &Mixer) {
        self.cut.store(mixer.0.clock(), Ordering::Release);
    }
}

/// What each channel takes of a phrase heard from outside, now, with the
/// two together (the gain before its sides) and how far it is: `level` (the
/// overall volume, the speech preset's, the clip's and its group's gain,
/// multiplied) times `falling` at the distance from the listener (`ear`) to
/// the head (the speech preset's curve over the phrase's farthest), each
/// side by where the head lies of the listener's right (`pan`).
pub(crate) fn speech_gains(head: Vec3, ear: Vec3, right: Vec3, level: f32, falling: impl Fn(f32) -> f32) -> ([f32; 2], f32, f32) {
    let to = head - ear;
    let distance = to.length();
    let gain = level * falling(distance).max(0.0);
    (pan(to, right).map(|side| side * gain), gain, distance)
}

/// A dummy's voice once read: its speaker, each bank's clips (the sound, its
/// volume and falloff), what plays, and what its speaker was last told.
struct Voiced {
    name: String,
    speaker: Speaker,
    clips: Vec<Vec<(Option<Samples>, f32, f32)>>,
    playing: Option<Playing>,
    parts: [f32; 7],
    audible: bool,
    shield: Option<(bool, bool)>,
}

/// Every dummy's voice, by its number, as they are read.
#[derive(Resource)]
pub struct DummyVoices {
    names: Vec<Option<String>>,
    voices: Vec<Option<Voiced>>,
    coming: Option<Mutex<Receiver<Read>>>,
    speech: Option<AudioGroupPreset>,
    character: Option<AudioGroupPreset>,
    crack: Option<Samples>,
    /// `StaminaExhaustionStartsBreathSound`.
    exhaustion_heard: bool,
    seed: u64,
    logged: u32,
}

/// Starts reading each dummy's voice away from the frame (`names`, by its
/// number; none for one with no voice), if the options' `HEALTH SYSTEM` and
/// `TARGETS` are on: off, nothing is read and nothing is said.
pub fn start(app: &mut App, install: trp_data::Install, on: bool, names: Vec<Option<String>>, seed: u64) {
    if !on || names.iter().all(Option::is_none) {
        return;
    }
    let exhaustion_heard = app.world().resource::<crate::GameData>().db.config.pointer("/Stamina/StaminaExhaustionStartsBreathSound").and_then(serde_json::Value::as_bool).unwrap_or(true);
    let (send, receive) = channel();
    let asked = names.clone();
    std::thread::spawn(move || {
        let presets = trp_data::audio::audio_preset::audio_group_presets(&install).map_err(|e| eprintln!("the audio groups' presets could not be read; the dummies' voices at the clips' own volume: {e:#}")).unwrap_or_default();
        let find = |group: i32| presets.iter().find(|p| p.group == group).cloned();
        let crack = trp_data::audio::sound::fracture_sound(&install).map_err(|e| eprintln!("a bone's crack could not be read; a dummy's is silent: {e:#}")).ok();
        let _ = send.send(Read::Common { speech: find(SPEECH), character: find(CHARACTER), crack });
        for (which, name) in asked.iter().enumerate() {
            let Some(name) = name else { continue };
            let read = (|| -> anyhow::Result<trp_data::audio::sound::VoiceBanks> {
                let mut reader = trp_data::audio::sound::VoiceReader::open(&install, name)?;
                // The health's banks, and what they chain to of their own.
                let health = reader.banks(&|t| HEALTH.contains(&t), &|_| false)?;
                let mut wanted = HEALTH.to_vec();
                wanted.extend(health.iter().filter(|b| b.chain.who == 0).map(|b| b.chain.event).filter(|t| ![trigger::NONE, trigger::PHRASE_NONE].contains(t)));
                let banks = reader.banks(&|t| wanted.contains(&t), &|_| true)?;
                Ok(trp_data::audio::sound::VoiceBanks { name: reader.name.clone(), bundle: reader.bundle.clone(), banks })
            })();
            let _ = send.send(Read::Voice(which, read.map_err(|e| format!("{e:#}"))));
        }
    });
    app.insert_resource(DummyVoices {
        voices: names.iter().map(|_| None).collect(),
        names,
        coming: Some(Mutex::new(receive)),
        speech: None,
        character: None,
        crack: None,
        exhaustion_heard,
        seed,
        logged: 0,
    });
    app.add_systems(Update, speak_dummies.run_if(resource_exists::<DummyVoices>).after(crate::weapons::bullets::fly_bullets).after(crate::world::dummy::shot_blast_dummies).before(crate::world::dummy::fall_dummies));
}

/// What a dummy's voice goes by now: alive, how hurt, its painkillers, and
/// whether its breath is heard (`PhysicalBase.BreathIsAudible`: not held,
/// and its legs' stamina, with the install's switch, or its breath's
/// exhausted).
fn situation(dummy: &crate::world::dummy::Dummy, exhaustion_heard: bool) -> Situation {
    let movement = dummy.movement();
    let audible = movement.is_some_and(|m| !m.stamina.holding_breath() && ((exhaustion_heard && m.stamina.legs.exhausted()) || m.stamina.oxygen.exhausted()));
    Situation {
        alive: dummy.health.alive,
        status: Status::of(&dummy.health),
        on_painkillers: movement.is_some_and(|m| m.conditions.on_painkillers),
        painkiller: dummy.health.active(Kind::PainKiller, None).is_some(),
        audible,
    }
}

/// The frame's voices, after the frame's bullets and blasts have reached
/// the dummies: each voice taken in as it is read; each dummy's speaker
/// told what happened to it in the game's order (the part a hit reached;
/// for each damage, the breathing asked as its health changed and then the
/// damage heard; an effect started or ended; its death; then each hit's
/// awareness), then whether its breath is heard and its face shield, then
/// its own update; what it says played from its head.
#[allow(clippy::too_many_arguments)]
pub fn speak_dummies(
    mut voices: ResMut<DummyVoices>,
    dummies: Option<ResMut<Dummies>>,
    rigs: Option<Res<DummyRigs>>,
    carried: Res<crate::screens::inventory_view::Carried>,
    game: Res<crate::GameData>,
    mixer: Option<Res<Mixer>>,
    levels: Option<Res<Levels>>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    helper: TransformHelper,
    (time, log): (Res<Time>, Option<Res<crate::audio::step_sound::SoundLog>>),
) {
    let voices = &mut *voices;
    let arrived: Vec<Read> = voices.coming.as_ref().and_then(|c| c.lock().ok()).map(|c| c.try_iter().collect()).unwrap_or_default();
    for read in arrived {
        match read {
            Read::Common { speech, character, crack } => {
                (voices.speech, voices.character) = (speech, character);
                voices.crack = crack.map(Samples::of_owned);
            }
            Read::Voice(which, Ok(voice)) => {
                let mut speaker = Speaker::of_ai(voices.seed.wrapping_add(which as u64 * 0x9e37_79b9) | 1);
                speaker.set_banks(voice.banks.iter().map(Bank::of).collect());
                let clips: Vec<Vec<(Option<Samples>, f32, f32)>> = voice.banks.into_iter().map(|b| b.clips.into_iter().map(|c| (c.clip.map(Samples::of_owned), c.volume, c.falloff)).collect()).collect();
                info!("dummy {which}'s voice {} ({}): {} banks, {} clips ({:?})", voice.name, voice.bundle, clips.len(), clips.iter().map(Vec::len).sum::<usize>(), speaker.triggers().collect::<Vec<_>>());
                if let Some(slot) = voices.voices.get_mut(which) {
                    *slot = Some(Voiced { name: voice.name, speaker, clips, playing: None, parts: [0.0; 7], audible: false, shield: None });
                }
            }
            Read::Voice(which, Err(e)) => {
                let name = voices.names.get(which).cloned().flatten().unwrap_or_default();
                eprintln!("dummy {which}'s voice {name} could not be read; it says nothing: {e}");
            }
        }
    }
    let Some(mut dummies) = dummies else { return };
    dummies.listened = true;
    dummies.hear_all();
    let camera = camera.single().ok();
    let overall = levels.as_deref().map_or(1.0, Levels::overall_gain);
    let now = time.elapsed_secs();
    for dummy in &mut dummies.list {
        let heard = dummy.take_heard();
        let which = dummy.number();
        let Some(voice) = voices.voices.get_mut(which).and_then(Option::as_mut) else { continue };
        let s = situation(dummy, voices.exhaustion_heard);
        let mut struck = Vec::new();
        let mut cracks = 0;
        for heard in heard {
            match heard {
                Heard::Anew => {
                    voice.speaker = std::mem::replace(&mut voice.speaker, Speaker::new(0)).renewed();
                    if let (Some(playing), Some(mixer)) = (voice.playing.take(), mixer.as_deref()) {
                        playing.cut(mixer);
                    }
                    (voice.parts, voice.audible, voice.shield) = ([0.0; 7], false, None);
                    struck.clear();
                }
                Heard::Struck(part, damage, kind) => {
                    voice.speaker.struck(part);
                    struck.push((part, damage, kind));
                }
                // Its health changed first (`HealthChangedEvent` before
                // `ApplyDamageEvent`): the breathing asked, then the damage.
                Heard::Health(Event::Damaged(_, damage, kind)) => {
                    if damage.abs() >= 0.01 {
                        voice.speaker.breath(now, &s);
                    }
                    voice.speaker.damaged(kind, damage, now, &s);
                }
                Heard::Health(Event::Started(kind, part)) => {
                    voice.speaker.effect_started(kind, part);
                    if kind == Kind::Fracture {
                        cracks += 1;
                    }
                    if kind == Kind::Berserk {
                        voice.speaker.berserk(now, &s);
                    }
                }
                Heard::Health(Event::Removed(kind, part, _)) => voice.speaker.effect_removed(kind, part),
                Heard::Health(Event::Died(kind)) => voice.speaker.died(kind, now, &s),
                Heard::Health(_) => {}
            }
        }
        // Each hit's own (`ApplyHitDebuff`): an enemy's makes it aware.
        for (part, damage, kind) in struck {
            voice.speaker.hit(part, damage, kind, now, &s);
        }
        let parts = Part::ALL.map(|p| dummy.health.part(p).current);
        if s.alive {
            if parts.iter().zip(voice.parts).any(|(now, was)| (now - was).abs() >= 0.01) {
                voice.speaker.breath(now, &s);
            }
            if s.audible != voice.audible {
                voice.audible = s.audible;
                voice.speaker.audible_changed(now, &s);
            }
            // Its headwear's and face cover's shields, as another's is seen
            // (`SetupSoundsFromFaceShieldComponent`): muffled while one is
            // down over armour on the jaw in that slot.
            let kit = carried.body(which);
            let slots = ["Headwear", "FaceCover"].map(|slot| kit.and_then(|kit| kit.in_slot(slot)).map_or((false, false), |worn| trp_sim::sound::speaker::shield_of(&game.db, worn)));
            let state = (slots.iter().any(|(down, _)| *down), slots.iter().any(|(_, muffles)| *muffles));
            if voice.shield != Some(state) {
                voice.shield = Some(state);
                voice.speaker.face_shield_seen(state.0, state.1);
            }
        }
        voice.parts = parts;
        voice.speaker.update(now, &s);
        let said = voice.speaker.take_said();
        // A phrase whose clip is over is followed no more.
        if voice.playing.as_ref().is_some_and(|playing| now >= playing.until) {
            voice.playing = None;
        }
        let (Some(mixer), Some(camera)) = (mixer.as_deref(), camera) else { continue };
        let ear = camera.translation();
        let right = camera.right().as_vec3();
        // Its head where its newest frame put it, not as drawn between two
        // of its frames (`Dummy::bone_now`): where the voice is placed
        // decides whether it is played. Asked only when it says something
        // or a phrase of its plays (its pose made then, where it was not).
        let head = if said.is_empty() && voice.playing.is_none() { None } else { rigs.as_deref().and_then(|rigs| rigs.0.get(which)).and_then(|rig| dummy.bone_now(rig, rig.bone(HEAD)?, &helper)).map(|g| Vec3::from(g.translation)) };
        let outdoors = dummy.movement().map_or(1.0, |m| m.sounds.settings.outdoor_rolloff);
        // The volume a phrase is played at before its distance (the overall
        // volume, the speech preset's, the clip's, its group's gain), and
        // the preset's curve at a distance over a phrase's farthest.
        let speech = voices.speech.as_ref();
        let base = speech.map_or(1.0, |p| p.overall_volume);
        let level = |volume: f32, muffled: bool| overall * base * volume * mixer.0.observed_speech_gain(muffled);
        let falling = |farthest: f32| move |distance: f32| speech.map_or(1.0, |p| p.rolloff_gain(distance, farthest));
        // The phrase playing goes with the head: its gains as the head and
        // the listener are now, told to the mix.
        if let (Some(playing), Some(head)) = (voice.playing.as_mut(), head) {
            let (gains, _, distance) = speech_gains(head, ear, right, level(playing.volume, playing.muffled), falling(playing.farthest));
            set_gains(&playing.gains, gains);
            // In the sounds' log where a side has moved a tenth of the
            // whole since its last line.
            let whole = gains[0].hypot(gains[1]).max(playing.logged[0].hypot(playing.logged[1]));
            if gains.into_iter().zip(playing.logged).any(|(new, was)| (new - was).abs() > 0.1 * whole) {
                playing.logged = gains;
                log_follow(&log, now, which, &voice.name, gains, distance);
            }
        }
        for said in said {
            let phrase = match said {
                Said::Release => {
                    if let Some(playing) = voice.playing.take() {
                        playing.cut(mixer);
                    }
                    continue;
                }
                Said::Phrase(phrase) => phrase,
            };
            let Some((Some(samples), volume, falloff)) = voice.clips.get(phrase.bank).and_then(|b| b.get(phrase.clip)) else { continue };
            let Some(head) = head else { continue };
            let farthest = falloff * outdoors;
            let (gains, gain, distance) = speech_gains(head, ear, right, level(*volume, phrase.muffled), falling(farthest));
            let trigger = voice.speaker.triggers().nth(phrase.bank).unwrap_or(0);
            // Past its reach as it begins the phrase is said but not
            // played: what the source plays goes on.
            if distance > farthest + CHECK_RANGE_BUFF {
                log_line(&log, now, which, &voice.name, trigger, phrase.clip, *volume, 0.0, farthest, distance, "out of reach");
                continue;
            }
            if let Some(playing) = voice.playing.take() {
                playing.cut(mixer);
            }
            let group = if phrase.muffled { Group::SimpleOccluded } else { Group::ObservedPlayer };
            let playing = Playing { cut: no_cut(), gains: gains_of(gains), until: now + samples.seconds() as f32, volume: *volume, farthest, muffled: phrase.muffled, logged: gains };
            mixer.0.add_to(group, Voice::placed(samples, gains, Arc::new(AtomicU32::new(1f32.to_bits()))).following_gains(playing.gains.clone()).at_frame(mixer.0.clock(), false, playing.cut.clone()));
            voice.playing = Some(playing);
            log_line(&log, now, which, &voice.name, trigger, phrase.clip, *volume, gain, farthest, distance, if phrase.muffled { "muffled" } else { "" });
            if voices.logged < 64 {
                voices.logged += 1;
                info!("dummy {which}'s voice: phrase {trigger} clip {} at {gain:.4}, {distance:.1} m off{}", phrase.clip, if phrase.muffled { ", muffled" } else { "" });
            }
        }
        // A bone's crack where its feet are, by the character group's
        // rolloff over 15 m (`PlayAtPoint`).
        if let (Some(crack), Some(feet)) = (voices.crack.as_ref(), dummy.movement().map(|m| m.position)) {
            for _ in 0..cracks {
                let to = Vec3::new(feet[0], feet[1], -feet[2]) - ear;
                let distance = to.length();
                if distance > CRACK_ROLLOFF {
                    continue;
                }
                let falloff = voices.character.as_ref().map_or(1.0, |p| p.rolloff_gain(distance, CRACK_ROLLOFF).max(0.0) * p.overall_volume);
                let [left, right] = pan(to, right);
                let gain = overall * falloff;
                mixer.0.add_to(Group::CommonSounds, Voice::placed(crack, [left * gain, right * gain], Arc::new(AtomicU32::new(1f32.to_bits()))));
                info!("dummy {which}'s bone's crack, {distance:.1} m off");
            }
        }
    }
}

/// A line of the sounds' log (`--log-sounds`), for a check by hand.
#[allow(clippy::too_many_arguments)]
fn log_line(log: &Option<Res<crate::audio::step_sound::SoundLog>>, now: f32, which: usize, name: &str, trigger: i32, clip: usize, volume: f32, gain: f32, farthest: f32, distance: f32, note: &str) {
    if let Some(Ok(mut out)) = log.as_ref().map(|log| log.0.lock()) {
        use std::io::Write;
        let _ = writeln!(out, "{now:9.3} dummy {which} voice {name} phrase {trigger} clip {clip} volume {volume:.2} gain {gain:.4} farthest {farthest:.1} distance {distance:.3} {note}");
        let _ = out.flush();
    }
}

/// A line of the sounds' log for a phrase playing whose gains have moved:
/// what each channel takes of it now, and how far its head is.
fn log_follow(log: &Option<Res<crate::audio::step_sound::SoundLog>>, now: f32, which: usize, name: &str, gains: [f32; 2], distance: f32) {
    if let Some(Ok(mut out)) = log.as_ref().map(|log| log.0.lock()) {
        use std::io::Write;
        let _ = writeln!(out, "{now:9.3} dummy {which} voice {name} follows left {:.4} right {:.4} distance {distance:.3}", gains[0], gains[1]);
        let _ = out.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A curve as the speech preset's is laid: one at the source, nought at
    /// the farthest and past it (here a straight line, over 60 m).
    fn falling(distance: f32) -> f32 {
        1.0 - (distance / 60.0).min(1.0)
    }

    /// A head ahead of a listener is heard alike on both sides; the
    /// listener turned a quarter to its left has it wholly on its right,
    /// turned the other way wholly on its left, at the same gain.
    #[test]
    fn the_side_changes_when_the_listener_turns() {
        let (head, ear) = (Vec3::new(0.0, 1.6, -10.0), Vec3::new(0.0, 1.6, 0.0));
        let (ahead, gain, distance) = speech_gains(head, ear, Vec3::X, 0.5, falling);
        assert!((distance - 10.0).abs() < 1e-5 && (gain - 0.5 * (50.0 / 60.0)).abs() < 1e-6, "{distance} {gain}");
        assert!((ahead[0] - ahead[1]).abs() < 1e-6 && (ahead[0] - gain * std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6, "{ahead:?}");
        // Facing +X, the listener's right is +Z, away from the head: left.
        let (left, turned, _) = speech_gains(head, ear, Vec3::Z, 0.5, falling);
        assert!((left[0] - gain).abs() < 1e-6 && left[1].abs() < 1e-6 && turned == gain, "{left:?}");
        // Facing -X, its right is -Z, towards the head: right.
        let (right, ..) = speech_gains(head, ear, Vec3::NEG_Z, 0.5, falling);
        assert!(right[0].abs() < 1e-6 && (right[1] - gain).abs() < 1e-6, "{right:?}");
        // Half way round, between: more on the right, the power the same.
        let half = Vec3::new(1.0, 0.0, -1.0).normalize();
        let (between, ..) = speech_gains(head, ear, half, 0.5, falling);
        assert!(between[1] > between[0] && between[0] > 0.0, "{between:?}");
        assert!((between[0] * between[0] + between[1] * between[1] - gain * gain).abs() < 1e-6);
    }

    /// The listener walking off hears it fall by the curve, to nought at
    /// its farthest and past it (never under); the head moving does the
    /// same, and puts it on the side it went to.
    #[test]
    fn the_loudness_changes_when_the_listener_walks_off() {
        let head = Vec3::new(0.0, 1.6, -10.0);
        let at = |ear: Vec3| speech_gains(head, ear, Vec3::X, 1.0, falling);
        let (near, far, farthest, past) = (at(Vec3::new(0.0, 1.6, 0.0)).1, at(Vec3::new(0.0, 1.6, 20.0)).1, at(Vec3::new(0.0, 1.6, 50.0)).1, at(Vec3::new(0.0, 1.6, 90.0)));
        assert!((near - 50.0 / 60.0).abs() < 1e-6 && (far - 0.5).abs() < 1e-6, "{near} {far}");
        assert_eq!((farthest, past.1, past.0), (0.0, 0.0, [0.0, 0.0]));
        // A curve that dips under nought gives nought.
        assert_eq!(speech_gains(head, Vec3::ZERO, Vec3::X, 1.0, |_| -0.2).1, 0.0);
        // The head gone 10 m to the listener's right: farther, and there.
        let (moved, gain, distance) = speech_gains(head + Vec3::X * 10.0, Vec3::new(0.0, 1.6, 0.0), Vec3::X, 1.0, falling);
        assert!((distance - 200f32.sqrt()).abs() < 1e-4 && gain < near && moved[1] > moved[0], "{moved:?} {gain} {distance}");
    }
}
