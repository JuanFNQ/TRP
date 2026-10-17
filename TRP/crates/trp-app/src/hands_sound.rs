//! The sounds of what is in the hands (`docs/spec/sound.md`, "The hands'
//! sounds"): a weapon's, an item's or the empty hands' own sound player
//! (`BaseSoundPlayer`, read with the rig: `trp_data::sound::hands_sounds`)
//! plays the clip its animator's `Sound` marks name (`OnSound`,
//! `SoundEventHandler`): the element of its `AdditionalSounds` named by the
//! mark, or by the mark with `Snd` before it, one of its clips at random, at
//! its volume.
//!
//! - **For the character's own eyes** the clip plays in stereo, whole
//!   (`PlayClip`: `forceStereo` in first person), a mono clip at equal
//!   power in the middle;
//! - **seen from outside** it plays from where the hands are, heard to the
//!   element's rolloff and not beyond (`PlayClip` returns past it): its
//!   level falls as one over the distance past 1 m and it is panned by
//!   how far to the side of the camera it lies (`PROVISIONAL`: the source's
//!   own curve and the engine's pan law are not read; the place is the
//!   character's, 1.4 m up, not the weapon's bone);
//! - at the overall volume; the mixer's `Weaponry` group adds nothing
//!   (`PROVISIONAL`: not read).
//!
//! With the options' `HANDS SOUNDS` off the players are not read and
//! nothing plays.

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use trp_sim::recoil::Rng;

use crate::controller::Controller;
use crate::ui_sound::{Levels, Mixer, Pitch, Samples, Voice};
use crate::weapon::HandsRig;
use crate::MainCamera;

/// A rig's sounds ready to play: each element of its `AdditionalSounds`
/// by its event's name, with its rolloff, volume and clips; and its shots
/// (`shot_sound`), where it has banks of them.
pub struct RigSounds {
    elements: Vec<Element>,
    pub shots: Option<Arc<crate::shot_sound::Shots>>,
    /// The weapon's kind of sound and its ergonomics, which its aiming's
    /// sound goes by; none for a rig that is no weapon.
    aiming: Option<(&'static str, f32)>,
}

/// Which of a rig's sounds are read: its marks' (`HANDS SOUNDS`), its
/// shots' (`SHOT SOUNDS`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Wanted {
    pub hands: bool,
    pub shots: bool,
}

impl Wanted {
    pub fn of(features: &crate::options_view::Features) -> Self {
        Self { hands: features.hands_sounds, shots: features.shot_sounds }
    }
}

struct Element {
    event: String,
    rolloff: f32,
    volume: f32,
    clips: Vec<Samples>,
    names: Vec<String>,
}

impl RigSounds {
    pub fn of(sounds: trp_data::sound::HandsSounds, wanted: Wanted, silenced: bool) -> Self {
        let elements = match wanted.hands {
            true => sounds
                .additional
                .into_iter()
                .map(|element| {
                    let names = element.clips.iter().map(|clip| clip.name.clone()).collect();
                    Element { event: element.event, rolloff: element.rolloff as f32, volume: element.volume, clips: element.clips.into_iter().map(Samples::of_owned).collect(), names }
                })
                .collect(),
            false => Vec::new(),
        };
        let shots = sounds.shots.filter(|_| wanted.shots).map(|shots| Arc::new(crate::shot_sound::Shots::of(shots, silenced)));
        Self { elements, shots, aiming: None }
    }

    /// The element a mark's string names (`SoundEventHandler`).
    fn for_mark(&self, name: &str) -> Option<&Element> {
        self.elements.iter().find(|element| element.event == name || element.event.strip_prefix("Snd") == Some(name))
    }

    pub fn len(&self) -> usize {
        self.elements.len()
    }
}

/// A rig's sound player read, if there is one and its sounds are wanted.
pub fn read(unity: &trp_data::unity::Assets, weapon: &trp_data::weapon::Weapon, setups: &mut trp_data::sound::VorbisSetups, wanted: Wanted) -> Option<Arc<RigSounds>> {
    if !wanted.hands && !wanted.shots {
        return None;
    }
    match trp_data::sound::hands_sounds(unity, weapon, setups, wanted.shots) {
        Ok(Some(sounds)) => {
            let player = sounds.player.clone();
            let mut rig = RigSounds::of(sounds, wanted, weapon.silenced);
            // A weapon's player plays its aiming's sound (`WeaponSoundPlayer.Init`).
            if player == "WeaponSoundPlayer" && wanted.hands {
                rig.aiming = weapon.sound_kind.map(|kind| (kind, weapon.total_ergonomics));
            }
            info!("{}: its {player}, {} sounds{}", weapon.name, rig.len(), if rig.shots.is_some() { ", and its shots" } else { "" });
            Some(Arc::new(rig))
        }
        Ok(None) => None,
        Err(e) => {
            warn!("the sounds of {}: {e:#}", weapon.name);
            None
        }
    }
}

/// Where the falloff seen from outside begins, in metres.
const FULL_WITHIN: f32 = 1.0;
/// How high over the feet the hands' sounds are taken to come from, seen
/// from outside (`PROVISIONAL`).
const HANDS_HIGH: f32 = 1.4;

/// The random draws of which clip plays.
#[derive(Resource)]
struct Draws(Rng);

/// The aiming's sounds (`trp_data::sound::aiming_sounds`), by the weapon's
/// kind, read away from the frame when `HANDS SOUNDS` is on; and each
/// bank's clips not yet played since all last were (`GetRandomClipWithExclusion`).
#[derive(Resource, Default)]
pub struct AimingBanks {
    banks: HashMap<String, Arc<AimingBank>>,
    left: HashMap<String, Vec<usize>>,
    coming: Option<Mutex<Receiver<ByKind>>>,
}

/// The banks by the weapons' kinds, as read.
type ByKind = Vec<(String, Arc<AimingBank>)>;

pub struct AimingBank {
    name: String,
    volume: f32,
    rolloff: f32,
    clips: Vec<(String, Samples)>,
}

/// Starts reading the aiming's sounds, if the hands' sounds are on.
pub fn start_reading(app: &mut App, install: trp_data::Install, on: bool) {
    let mut banks = AimingBanks::default();
    if on {
        let (send, receive) = channel();
        std::thread::spawn(move || match trp_data::sound::aiming_sounds(&install) {
            Ok(read) => {
                // Each bank made ready once, however many kinds share it.
                let mut made: HashMap<String, Arc<AimingBank>> = HashMap::new();
                let banks = read
                    .into_iter()
                    .map(|(kind, bank)| {
                        let ready = made
                            .entry(bank.name.clone())
                            .or_insert_with(|| Arc::new(AimingBank { name: bank.name.clone(), volume: bank.volume, rolloff: bank.rolloff, clips: bank.clips.iter().map(|clip| (clip.name.clone(), Samples::of_owned(clip.clone()))).collect() }))
                            .clone();
                        (kind, ready)
                    })
                    .collect();
                let _ = send.send(banks);
            }
            Err(e) => eprintln!("the aiming's sounds could not be read; they are silent: {e:#}"),
        });
        banks.coming = Some(Mutex::new(receive));
    }
    app.insert_resource(banks);
}

fn take_aiming(mut banks: ResMut<AimingBanks>) {
    let Some(coming) = banks.coming.take() else { return };
    let got = coming.lock().ok().and_then(|receive| match receive.try_recv() {
        Ok(read) => Some(Some(read)),
        Err(std::sync::mpsc::TryRecvError::Empty) => None,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(None),
    });
    match got {
        None => banks.coming = Some(coming),
        Some(Some(read)) => {
            info!("the aiming's sounds: {} kinds of weapon", read.len());
            banks.banks = read.into_iter().collect();
        }
        Some(None) => {}
    }
}

/// The aiming's sound's volume (`CalculateAimingSoundVolume`): the square
/// of how far the weapon's ergonomics fall short of 100, as a share,
/// within 0.1 and 0.2; times the equipment's noise (1 and the overweight,
/// less the Covert Movement buff); the draw's skill and the bots' sound
/// coefficient untrained (0).
fn aiming_volume(ergonomics: f32, overweight: f32) -> f32 {
    let short = ergonomics / 100.0 - 1.0;
    (short * short).clamp(0.1, 0.2) * (1.0 + overweight)
}

/// The clip of a bank to play next (`GetRandomClipWithExclusion`): one of
/// those not played since all last were, drawn by an integer
/// `Random.Range(0, n - 1)` among them (so never the last of them while
/// another is left), and set aside.
fn next_clip(left: &mut Vec<usize>, count: usize, rng: &mut Rng) -> Option<usize> {
    if count == 0 {
        return None;
    }
    if left.is_empty() {
        left.extend(0..count);
    }
    let top = left.len().saturating_sub(1);
    let at = if top == 0 { 0 } else { (rng.range(0.0, top as f32) as usize).min(top - 1) };
    Some(left.remove(at))
}

/// Plays the marks the hands passed this frame.
#[allow(clippy::too_many_arguments)]
fn play(
    mut controller: ResMut<Controller>,
    hands: Option<Res<HandsRig>>,
    mixer: Option<Res<Mixer>>,
    levels: Option<Res<Levels>>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    mut draws: ResMut<Draws>,
    log: Option<Res<crate::step_sound::SoundLog>>,
    time: Res<Time>,
    mut aiming: ResMut<AimingBanks>,
) {
    if controller.hands_sounds.is_empty() && controller.aims_up == 0 {
        return;
    }
    let marks = std::mem::take(&mut controller.hands_sounds);
    let aims = std::mem::take(&mut controller.aims_up);
    let (Some(hands), Some(mixer)) = (hands, mixer) else { return };
    let Some(sounds) = hands.sounds.as_ref() else { return };
    let overall = levels.map_or(1.0, |levels| levels.overall_gain());
    let own_eyes = controller.movement.body.point_of_view() == trp_sim::PointOfView::FirstPerson;
    // The sights brought up: the weapon's aiming sound (`PlayAimingSound`,
    // through `PlayClip` as the marks' sounds).
    if let Some((kind, ergonomics)) = sounds.aiming.filter(|_| aims > 0) {
        let aiming = &mut *aiming;
        if let Some(bank) = aiming.banks.get(kind).cloned() {
            let left = aiming.left.entry(bank.name.clone()).or_default();
            if let Some(at) = next_clip(left, bank.clips.len(), &mut draws.0) {
                let (name, clip) = &bank.clips[at];
                let gain = aiming_volume(ergonomics, controller.movement.stamina.settings.overweight) * bank.volume * overall;
                let placed = (!own_eyes).then(|| camera.single().ok()).flatten().map(|camera| {
                    let p = controller.movement.position;
                    let to_source = Vec3::new(p[0], p[1] + HANDS_HIGH, -p[2]) - camera.translation();
                    let distance = to_source.length();
                    let falloff = if distance > FULL_WITHIN { FULL_WITHIN / distance } else { 1.0 };
                    let side = if distance > 1e-4 { (to_source / distance).dot(camera.right().as_vec3()).clamp(-1.0, 1.0) } else { 0.0 };
                    let angle = (side + 1.0) * std::f32::consts::FRAC_PI_4;
                    (distance, [angle.cos() * falloff, angle.sin() * falloff])
                });
                if let Some(log) = &log {
                    use std::io::Write;
                    if let Ok(mut out) = log.0.lock() {
                        let _ = writeln!(out, "{:9.3} hands        {:28} {name} volume {gain:.4} {}", time.elapsed_secs(), format!("aiming ({kind})"), if own_eyes { "whole" } else { "placed" });
                        let _ = out.flush();
                    }
                }
                match placed {
                    None => mixer.0.add(Voice::whole(clip, gain)),
                    // Past the bank's rolloff it is not heard (`PlayClip`).
                    Some((distance, _)) if distance > bank.rolloff => {}
                    Some((_, gains)) => {
                        let pitch: Pitch = Arc::new(std::sync::atomic::AtomicU32::new(1.0f32.to_bits()));
                        mixer.0.add(Voice::placed(clip, [gains[0] * gain, gains[1] * gain], pitch));
                    }
                }
            }
        }
    }
    for mark in marks {
        let Some(element) = sounds.for_mark(&mark) else { continue };
        if element.clips.is_empty() {
            continue;
        }
        let drawn = (draws.0.range(0.0, element.clips.len() as f32) as usize).min(element.clips.len() - 1);
        let clip = &element.clips[drawn];
        let gain = element.volume * overall;
        if let Some(log) = &log {
            use std::io::Write;
            if let Ok(mut out) = log.0.lock() {
                let _ = writeln!(out, "{:9.3} hands        {:28} {} volume {gain:.4} {}", time.elapsed_secs(), element.event, element.names[drawn], if own_eyes { "whole" } else { "placed" });
                let _ = out.flush();
            }
        }
        if own_eyes {
            mixer.0.add(Voice::whole(clip, gain));
            continue;
        }
        let Ok(camera) = camera.single() else { continue };
        let p = controller.movement.position;
        let to_source = Vec3::new(p[0], p[1] + HANDS_HIGH, -p[2]) - camera.translation();
        let distance = to_source.length();
        if distance > element.rolloff {
            continue;
        }
        let falloff = if distance > FULL_WITHIN { FULL_WITHIN / distance } else { 1.0 };
        let side = if distance > 1e-4 { (to_source / distance).dot(camera.right().as_vec3()).clamp(-1.0, 1.0) } else { 0.0 };
        let angle = (side + 1.0) * std::f32::consts::FRAC_PI_4;
        let level = gain * falloff;
        let pitch: Pitch = Arc::new(std::sync::atomic::AtomicU32::new(1.0f32.to_bits()));
        mixer.0.add(Voice::placed(clip, [angle.cos() * level, angle.sin() * level], pitch));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The square of the ergonomics' shortfall, within 0.1 and 0.2: an
    /// AKM's 38 is at the top, a pistol's 90 at the floor, 60 between;
    /// the overweight adds to it as to the kit.
    #[test]
    fn the_aiming_s_volume_goes_by_the_ergonomics() {
        assert!((aiming_volume(38.0, 0.0) - 0.2).abs() < 1e-6);
        assert!((aiming_volume(90.0, 0.0) - 0.1).abs() < 1e-6);
        assert!((aiming_volume(60.0, 0.0) - 0.16).abs() < 1e-5);
        assert!((aiming_volume(38.0, 0.5) - 0.3).abs() < 1e-6);
    }

    /// Every clip once before any again, and never the last one left while
    /// another is (an integer's `Random.Range(0, n - 1)`).
    #[test]
    fn an_aiming_clip_is_not_played_again_until_all_have_been() {
        let mut rng = Rng::new(3);
        let mut left = Vec::new();
        for _ in 0..4 {
            let mut round: Vec<usize> = (0..5).map(|_| next_clip(&mut left, 5, &mut rng).expect("a clip")).collect();
            // The first drawn of a fresh round is never the list's last.
            assert_ne!(round[0], 4);
            round.sort();
            assert_eq!(round, [0, 1, 2, 3, 4]);
        }
        assert_eq!(next_clip(&mut Vec::new(), 1, &mut rng), Some(0));
        assert_eq!(next_clip(&mut Vec::new(), 0, &mut rng), None);
    }
}

pub struct HandsSoundPlugin;

impl Plugin for HandsSoundPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Draws(Rng::new(0x4841_4e44))).add_systems(Update, (take_aiming, play.after(crate::controller::step)));
    }
}
