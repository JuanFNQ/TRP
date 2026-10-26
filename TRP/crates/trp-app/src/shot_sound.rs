//! The shots' sounds (`docs/spec/sound.md`, "The shots"): a weapon's own
//! sound player (`WeaponSoundPlayer`, read with the rig) plays each shot
//! from its banks through a queue of two sources (`SuperBetterAudioQueue`),
//! scheduled on the mix's own clock (the game's `dspTime`):
//!
//! - **The bank**: `Body`, or `Doublet` for two chambers fired together;
//!   with a silencer on the silenced ones (`BodySilenced` and
//!   `TailSilenced` falling back to their plain banks, `DoubletSilenced`
//!   not). Its clip by the distance (`PickClipsByDistance`): under its
//!   first blend value the close list's alone, between two values two
//!   lists' clips blended, each list's clip dealt from the bank's shuffled
//!   deck (`GetRandomClipIndex`).
//! - **An automatic weapon** (not `Non_auto`): the first shot starts its
//!   body as a loop, and further shots while it plays only nudge its pitch;
//!   the trigger let go (`BreakLoop`), or not held at the shot, schedules
//!   the tail on the next beat (a sixteenth of the bank's length) and ends
//!   the loop there. **Any other**: each shot's body whole.
//! - **Each sample queued ends the ones before it** at its start (the
//!   queue's other source is given that end: `SetScheduledEndTime`).
//! - **The pitch**: each shot's `1 ± 0.03` at random (`CalculateBaseAimingVolume`;
//!   the overheat that would set it otherwise is not built), within 0.965 to
//!   1.045 once the loop plays; between the trigger let go and the tail it
//!   is drawn toward 1 each frame (`SmoothResetPitch`).
//! - **Heard**: for the character's own eyes in stereo, whole; seen from
//!   outside from the character's place 1.4 m up, after the sound's travel
//!   (340.29 m/s), falling as one over the distance past 1 m and panned as
//!   the hands' sounds (`PROVISIONAL`, as theirs); at the bank's volume and
//!   the overall volume, through the `Gunshots` group, read at 0.40.0: under
//!   `Guns`, `World`, `InGame` and `Master`, all 0 dB (`gunshots_gain`).
//!
//! Not built: the supersonic crack heard from outside (`FireSonicSound`),
//! occlusion, the indoor clips (the range has no indoors), the burst of
//! two's own rules, a malfunction's break (none is built).
//!
//! With the options' `SHOT SOUNDS` off the banks are not read and the
//! shots are silent.

use std::sync::Arc;

use bevy::prelude::*;
use trp_sim::recoil::Rng;

use crate::controller::Controller;
use crate::ui_sound::{no_cut, Cut, Levels, Mixer, Pitch, Samples, Voice, MIX_RATE};
use crate::weapon::HandsRig;
use crate::MainCamera;

/// What the controller's weapon asked of its sound player, in order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShotCall {
    /// A shot made (`FireBullet`): whether the trigger was held then, and
    /// whether two chambers fired together.
    Fire { trigger: bool, doublet: bool },
    /// The trigger let go (`BreakLoop`).
    Release,
}

/// A weapon's banks of shots ready to play.
pub struct Shots {
    non_auto: bool,
    silenced: bool,
    /// By `trp_data::sound::SHOT_BANKS`' order.
    banks: Vec<Option<Bank>>,
}

struct Bank {
    name: String,
    volume: f32,
    clip_length: f32,
    blend: Option<Vec<f32>>,
    lists: [Vec<(String, Samples)>; 3],
}

const BODY: usize = 0;
const TAIL: usize = 1;
const DOUBLET: usize = 2;
const BODY_SILENCED: usize = 3;
const TAIL_SILENCED: usize = 4;
const DOUBLET_SILENCED: usize = 5;

impl Shots {
    pub fn of(shots: trp_data::sound::ShotSounds, silenced: bool) -> Self {
        let clips: Vec<Option<(String, Samples)>> = shots.clips.into_iter().map(|(name, clip)| clip.map(|clip| (name, Samples::of_owned(clip)))).collect();
        let banks = shots
            .banks
            .into_iter()
            .map(|bank| {
                bank.map(|bank| Bank {
                    name: bank.name,
                    volume: bank.base_volume,
                    clip_length: bank.clip_length,
                    blend: bank.blend,
                    lists: bank.lists.map(|list| list.into_iter().filter_map(|at| clips.get(at).cloned().flatten()).collect()),
                })
            })
            .collect();
        Self { non_auto: shots.non_auto, silenced, banks }
    }

    fn bank(&self, at: usize) -> Option<&Bank> {
        self.banks.get(at).and_then(Option::as_ref)
    }

    /// The body's bank, or the doublet's, as the silencer has it (`IsSilenced`:
    /// the silenced body and tail fall back to the plain ones).
    fn shot_bank(&self, doublet: bool) -> Option<usize> {
        let at = match (doublet, self.silenced) {
            (false, false) => BODY,
            (false, true) if self.bank(BODY_SILENCED).is_some() => BODY_SILENCED,
            (false, true) => BODY,
            (true, false) => DOUBLET,
            (true, true) => DOUBLET_SILENCED,
        };
        self.bank(at).map(|_| at)
    }

    fn tail_bank(&self) -> Option<usize> {
        let at = if self.silenced && self.bank(TAIL_SILENCED).is_some() { TAIL_SILENCED } else { TAIL };
        self.bank(at).map(|_| at)
    }

    /// A beat of the loop: a sixteenth of the body's length (`BeatLn`).
    fn beat(&self) -> f64 {
        let body = self.shot_bank(false).and_then(|at| self.bank(at));
        body.map_or(0.0, |bank| bank.clip_length as f64 / 16.0)
    }
}

/// A bank's deck of its clips (`SoundBank.Shuffle`, `GetRandomClipIndex`):
/// dealt from its end, shuffled afresh when it is out.
#[derive(Default, Clone)]
struct Deck {
    order: Vec<usize>,
    left: usize,
}

impl Deck {
    fn deal(&mut self, length: usize, rng: &mut Rng) -> usize {
        if length <= 1 {
            return 0;
        }
        if self.left < 1 {
            if self.order.len() < length {
                self.order.resize(length, 0);
            }
            for (n, slot) in self.order.iter_mut().enumerate().take(length) {
                *slot = n;
            }
            // `Random.Range(0, length - 1)`, an integer's: up to length - 2.
            for n in 0..length {
                let other = (rng.range(0.0, (length - 1) as f32) as usize).min(length - 2);
                self.order.swap(other, n);
            }
            self.left = length;
        }
        self.left -= 1;
        self.order[self.left] % length
    }
}

/// Clips picked for a distance: the first's, the second's if two lists
/// blend there, and the first's share (`PickClipsByDistance`; a bank
/// without blend values leaves the share as it was).
fn pick<'a>(bank: &'a Bank, deck: &mut Deck, rng: &mut Rng, distance: f32) -> (Option<&'a Samples>, Option<&'a Samples>, Option<f32>) {
    let mut deal = |list: usize| {
        let clips = &bank.lists[list];
        (!clips.is_empty()).then(|| &clips[deck.deal(clips.len(), rng) % clips.len()].1)
    };
    let Some(blend) = &bank.blend else { return (deal(0), None, None) };
    let i = blend.iter().take_while(|value| distance >= **value).count();
    let (first, second) = match i {
        0 => (deal(0), None),
        1 => (deal(0), deal(1)),
        2 => (deal(1), None),
        3 => (deal(1), deal(2)),
        _ => (deal(2), None),
    };
    let share = if i % 2 != 0 {
        let (a, b) = (blend[i], blend[i - 1]);
        if (b - a).abs() > f32::EPSILON { ((distance - a) / (b - a)).clamp(0.0, 1.0) } else { 1.0 }
    } else {
        1.0
    };
    (first, second, Some(share))
}

fn length(clip: Option<&Samples>) -> f64 {
    clip.map_or(0.0, |clip| clip.seconds())
}

/// The speed of sound the game's shots travel at (`SOUND_SPEED`).
const SOUND_SPEED: f32 = 340.29;
/// The pitch's bounds once the loop plays (`MIN_ALLOWED_PITCH`, `MAX_ALLOWED_PITCH`).
const PITCH_BOUNDS: (f32, f32) = (0.965, 1.045);
/// How high over the feet the shots are taken to come from (`PROVISIONAL`,
/// as the hands' sounds).
const HANDS_HIGH: f32 = 1.4;
/// Where the falloff seen from outside begins, in metres (`PROVISIONAL`).
const FULL_WITHIN: f32 = 1.0;

/// The sound player's state (`WeaponSoundPlayer`'s fields) for the rig in
/// hand.
#[derive(Resource)]
struct Player {
    /// The rig's banks this state is for.
    shots: Option<Arc<Shots>>,
    /// The queue borrowed: its sources' pitch and every sample queued on it
    /// with its cut.
    queue: Option<Queue>,
    firing: bool,
    start_fire: f64,
    delay: f64,
    start_playing: f64,
    release_time: f64,
    pitch: f32,
    prev_pitch: f32,
    balance: f32,
    decks: Vec<Deck>,
    rng: Rng,
}

struct Queue {
    pitch: Pitch,
    cuts: Vec<Cut>,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            shots: None,
            queue: None,
            firing: false,
            start_fire: 0.0,
            delay: 0.0,
            start_playing: 0.0,
            release_time: 0.0,
            pitch: 1.0,
            prev_pitch: 1.0,
            balance: 1.0,
            decks: vec![Deck::default(); trp_data::sound::SHOT_BANKS.len()],
            rng: Rng::new(0x5348_4f54),
        }
    }
}

/// How the frame hears the weapon: from the character's own eyes, or from
/// a place with a distance and the gains of the two channels.
#[derive(Clone, Copy)]
struct Heard {
    own_eyes: bool,
    distance: f32,
    gains: [f32; 2],
}

impl Player {
    /// `UpdatePitch`: within bounds, set on the queue if it has moved.
    fn update_pitch(&mut self, pitch: f32) {
        let Some(queue) = &self.queue else { return };
        let pitch = pitch.clamp(PITCH_BOUNDS.0, PITCH_BOUNDS.1);
        if (pitch - self.prev_pitch).abs() > f32::EPSILON {
            self.pitch = pitch;
            self.prev_pitch = pitch;
            queue.pitch.store(pitch.to_bits(), std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// A sample queued (`Enqueue`): the clips from `start` (seconds of the
    /// mix), for `seconds` or in a loop if 0, the samples before it ended
    /// at its start.
    #[allow(clippy::too_many_arguments)]
    fn enqueue(&mut self, mixer: &Mixer, clips: (Option<&Samples>, Option<&Samples>), start: f64, seconds: f64, volume: f32, heard: Heard, log: &mut Vec<String>) {
        let Some(queue) = &mut self.queue else { return };
        let starts = (start * MIX_RATE as f64).round().max(0.0) as u64;
        for cut in &queue.cuts {
            cut.fetch_min(starts, std::sync::atomic::Ordering::AcqRel);
        }
        let cut = no_cut();
        if seconds != 0.0 {
            cut.store(starts + (seconds * MIX_RATE as f64).round() as u64, std::sync::atomic::Ordering::Release);
        }
        let looping = seconds == 0.0;
        for (clip, share) in [(clips.0, self.balance), (clips.1, 1.0 - self.balance)] {
            let Some(clip) = clip else { continue };
            let gain = share * volume * mixer.0.gunshots_gain();
            let voice = if heard.own_eyes {
                Voice::whole(clip, gain).following(queue.pitch.clone())
            } else {
                Voice::placed(clip, [heard.gains[0] * gain, heard.gains[1] * gain], queue.pitch.clone())
            };
            mixer.0.add_to(crate::ui_sound::Group::Guns, voice.at_frame(starts, looping, cut.clone()));
        }
        queue.cuts.push(cut);
        log.push(format!("from {start:.4} s, {}, share {:.3}", if looping { "looping".to_owned() } else { format!("{seconds:.4} s") }, self.balance));
    }

    /// `FireBullet`.
    #[allow(clippy::too_many_arguments)]
    fn fire(&mut self, mixer: &Mixer, now: f64, trigger: bool, doublet: bool, heard: Heard, gain: f32, log: &mut Vec<String>) {
        let Some(shots) = self.shots.clone() else { return };
        let Some(at) = shots.shot_bank(doublet) else { return };
        let bank = shots.bank(at).expect("a bank found");
        // `CalculateBaseAimingVolume`: the overheat's own rate is not built.
        let pitch_mult = 1.0 + self.rng.range(-0.03, 0.03);
        let automatic = !shots.non_auto;
        if self.firing {
            if self.queue.is_some() {
                self.update_pitch(pitch_mult);
            }
            return;
        }
        if self.queue.is_none() {
            self.queue = Some(Queue { pitch: Arc::new(std::sync::atomic::AtomicU32::new(pitch_mult.to_bits())), cuts: Vec::new() });
        }
        if automatic {
            self.firing = true;
        }
        let (first, second, share) = pick(bank, &mut self.decks[at], &mut self.rng, heard.distance);
        if let Some(share) = share {
            self.balance = share;
        }
        self.pitch = pitch_mult;
        self.prev_pitch = pitch_mult;
        if let Some(queue) = &self.queue {
            queue.pitch.store(pitch_mult.to_bits(), std::sync::atomic::Ordering::Relaxed);
        }
        let seconds = length(first).max(length(second));
        self.delay = (heard.distance / SOUND_SPEED) as f64;
        self.start_fire = now;
        self.release_time = now + seconds + 1.0;
        let whole = if shots.non_auto { seconds / self.pitch as f64 } else { 0.0 };
        log.push(format!("shot: {} ({}), trigger {}", bank.name, if doublet { "both chambers" } else { "one chamber" }, if trigger { "held" } else { "let go" }));
        self.enqueue(mixer, (first, second), self.start_fire + self.delay, whole, bank.volume * gain, heard, log);
        if !trigger && automatic {
            self.update_pitch(pitch_mult);
            self.tail(mixer, &shots, 1, heard, gain, log);
        }
    }

    /// The tail after so many beats from the loop's start (`StopFiringLoop`,
    /// and `FireBullet` with the trigger let go).
    fn tail(&mut self, mixer: &Mixer, shots: &Shots, beats: u64, heard: Heard, gain: f32, log: &mut Vec<String>) {
        let beat = shots.beat();
        self.start_playing = self.start_fire + self.delay + beats as f64 * beat / self.pitch as f64;
        self.firing = false;
        let Some(at) = shots.tail_bank() else { return };
        let bank = shots.bank(at).expect("a bank found");
        let (first, second, share) = pick(bank, &mut self.decks[at], &mut self.rng, heard.distance);
        if let Some(share) = share {
            self.balance = share;
        }
        let seconds = length(first).max(length(second)) / self.pitch as f64;
        log.push(format!("tail: {} after {beats} beats of {beat:.4} s", bank.name));
        self.enqueue(mixer, (first, second), self.start_playing, seconds, bank.volume * gain, heard, log);
        self.release_time = self.start_playing + seconds;
    }

    /// `OnBreakLoop`: the loop ended on the beat after now.
    fn break_loop(&mut self, mixer: &Mixer, now: f64, heard: Heard, gain: f32, log: &mut Vec<String>) {
        if !self.firing {
            return;
        }
        let Some(shots) = self.shots.clone() else { return };
        let beat = shots.beat();
        let beats = if beat > 0.0 { ((now - self.start_fire) / beat).max(0.0) as u64 + 1 } else { 1 };
        self.tail(mixer, &shots, beats, heard, gain, log);
    }

    /// The frame's own work (`Update`): the queue given back when all it
    /// played is over, and until then, the trigger let go, the pitch drawn
    /// toward 1 (`SmoothResetPitch`).
    fn update(&mut self, now: f64) {
        if self.queue.is_none() || self.firing {
            return;
        }
        if now > self.release_time {
            self.queue = None;
            return;
        }
        let t = (now - self.start_fire) / (self.start_playing - now);
        let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) as f32 };
        let pitch = self.pitch + (1.0 - self.pitch) * t;
        self.update_pitch(pitch);
    }
}

/// The weapon in hand as the frame hears it.
fn heard_now(controller: &Controller, camera: &GlobalTransform) -> Heard {
    let own_eyes = controller.movement.body.point_of_view() == trp_sim::PointOfView::FirstPerson;
    let p = controller.movement.position;
    let to_source = Vec3::new(p[0], p[1] + HANDS_HIGH, -p[2]) - camera.translation();
    let distance = to_source.length();
    let falloff = if distance > FULL_WITHIN { FULL_WITHIN / distance } else { 1.0 };
    let side = if distance > 1e-4 { (to_source / distance).dot(camera.right().as_vec3()).clamp(-1.0, 1.0) } else { 0.0 };
    let angle = (side + 1.0) * std::f32::consts::FRAC_PI_4;
    Heard { own_eyes, distance, gains: [angle.cos() * falloff, angle.sin() * falloff] }
}

/// Plays what the weapon asked this frame, and keeps the player's state.
#[allow(clippy::too_many_arguments)]
fn play(
    mut controller: ResMut<Controller>,
    hands: Option<Res<HandsRig>>,
    mixer: Option<Res<Mixer>>,
    levels: Option<Res<Levels>>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    mut player: ResMut<Player>,
    log: Option<Res<crate::step_sound::SoundLog>>,
    time: Res<Time>,
) {
    let calls = std::mem::take(&mut controller.shot_calls);
    let Some(mixer) = mixer else { return };
    let Ok(camera) = camera.single() else { return };
    let now = mixer.0.clock() as f64 / MIX_RATE as f64;
    let heard = heard_now(&controller, camera);
    let gain = levels.map_or(1.0, |levels| levels.overall_gain());
    let mut said = Vec::new();
    // Another rig in hand: the one before is destroyed (`OnDestroy`), its
    // loop ended on the beat and left to play out.
    let shots = hands.as_ref().and_then(|hands| hands.sounds.as_ref()).and_then(|sounds| sounds.shots.clone());
    let same = match (&player.shots, &shots) {
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    };
    if !same {
        player.break_loop(&mixer, now, heard, gain, &mut said);
        *player = Player { shots, rng: std::mem::replace(&mut player.rng, Rng::new(0)), ..Default::default() };
    }
    // Dead, the loop ends as the trigger's does (`OnPlayerDead`).
    let dead = controller.health.as_ref().is_some_and(|health| !health.alive);
    for call in calls {
        match call {
            ShotCall::Fire { trigger, doublet } => player.fire(&mixer, now, trigger, doublet, heard, gain, &mut said),
            ShotCall::Release => player.break_loop(&mixer, now, heard, gain, &mut said),
        }
    }
    if dead {
        player.break_loop(&mixer, now, heard, gain, &mut said);
    }
    player.update(now);
    if let Some(log) = &log {
        use std::io::Write;
        if let Ok(mut out) = log.0.lock() {
            for line in said {
                let _ = writeln!(out, "{:9.3} shots        {line} (mix at {now:.4} s, {})", time.elapsed_secs(), if heard.own_eyes { "whole" } else { "placed" });
            }
            let _ = out.flush();
        }
    }
}

pub struct ShotSoundPlugin;

impl Plugin for ShotSoundPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Player>().add_systems(Update, play.after(crate::controller::step));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two clips' deck deals them in turn, the first second (an integer's
    /// `Random.Range(0, 1)` is always 0), whatever the draws.
    #[test]
    fn a_deck_of_two_alternates() {
        let mut deck = Deck::default();
        let mut rng = Rng::new(7);
        let dealt: Vec<usize> = (0..6).map(|_| deck.deal(2, &mut rng)).collect();
        assert_eq!(dealt, vec![0, 1, 0, 1, 0, 1]);
        // One clip is always the first.
        assert_eq!(deck.deal(1, &mut rng), 0);
    }

    /// The AKM's blend values, 5, 30, 55 and 100 m: the close clip alone
    /// under 5 m, the close and the distant blended to 30, the distant
    /// alone to 55.
    #[test]
    fn a_shot_is_picked_by_its_distance() {
        let clip = |name: &str| (name.to_owned(), Samples::of(&trp_data::sound::Clip { name: name.into(), channels: 1, frequency: 44100, samples: vec![0; 10] }));
        let bank = Bank { name: "akm_main".into(), volume: 1.0, clip_length: 1.6, blend: Some(vec![5.0, 30.0, 55.0, 100.0]), lists: [vec![clip("close")], vec![clip("distant")], vec![clip("far")]] };
        let (mut deck, mut rng) = (Deck::default(), Rng::new(1));
        let (first, second, share) = pick(&bank, &mut deck, &mut rng, 0.5);
        assert!(first.is_some() && second.is_none());
        assert_eq!(share, Some(1.0));
        let (first, second, share) = pick(&bank, &mut deck, &mut rng, 17.5);
        assert!(first.is_some() && second.is_some());
        // Halfway from 5 to 30: the close clip's share a half.
        assert!((share.unwrap() - 0.5).abs() < 1e-6);
        let (_, second, share) = pick(&bank, &mut deck, &mut rng, 40.0);
        assert!(second.is_none());
        assert_eq!(share, Some(1.0));
    }
}
