//! The sounds of the player's own moving.
//! After `docs/spec/footsteps.md`: the install's banks for every surface and the kit (`trp_data::sound::
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
//!   by the `Character` group's rolloff curve (the install's preset, as a
//!   dummy's) at the distance over the source's farthest, which the rules
//!   set at 70 m times the kind's multiplier (`DEFAULT_FP_ROLLOFF`) times
//!   the outdoors' share (`OutdoorRolloffMult`: the range has no indoors),
//!   a vault's at its bank's own rolloff (`courses2`'s `7-sound-moving`
//!   and `7-sound-vault`: every source of one's own `Custom`, at that
//!   farthest); and it is panned by how far to the side of the camera it
//!   lies, at equal power (`PROVISIONAL`: the engine's pan law is not
//!   read).
//!
//! A corpse's fall at any death, the character's own and a dummy's, plays
//! from the same banks (`play_falls`; sound spec, "A body's fall").
//!
//! With the options' `MOVEMENT SOUNDS` off nothing is read and nothing
//! plays; the rules still run, as the bob's timing is theirs too.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use trp_data::audio::audio_preset::AudioGroupPreset;
use trp_data::audio::sound::MovementSounds;
use trp_sim::weapons::recoil::Rng;
use trp_sim::sound::steps::{choose, BankOf, Figures, Shuffle};

use crate::player::controller::Controller;
use crate::audio::ui_sound::{Levels, Mixer, Pitch, Samples, Voice};
use crate::MainCamera;

mod playing;

pub use playing::*;
use playing::{play, play_dummies, play_falls};

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
    /// The volume a corpse's fall is asked at (`trp_sim::landing::
    /// body_fall_volume`), once the drop sounds' curve is read.
    fall_volume: Option<f32>,
    coming: Option<Mutex<Receiver<Coming>>>,
}

/// What the reading brings: the banks, the `Character` group's preset and
/// a corpse's fall's volume.
type Coming = (MovementSounds, Option<AudioGroupPreset>, Option<f32>);

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
            fall_volume: None,
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
/// prefab `AudioSourceSpatialBuiltinEQReverbPrefab`), by which a sound
/// falls until the `Character` preset is read.
const FULL_WITHIN: f32 = 1.0;
/// How far one's own sources of moving reach at a kind's multiplier of 1
/// (`Player.DEFAULT_FP_ROLLOFF`).
const FIRST_PERSON_ROLLOFF: f32 = 70.0;

/// Starts reading the banks away from the frame, if the options' `MOVEMENT
/// SOUNDS` is on.
pub fn start_reading(app: &mut App, install: trp_data::Install, on: bool) {
    let mut banks = StepBanks::default();
    if on {
        let (send, receive) = channel();
        std::thread::spawn(move || match trp_data::audio::sound::movement_sounds(&install, None, true) {
            Ok(sounds) => {
                let character = match trp_data::audio::audio_preset::audio_group_presets(&install) {
                    Ok(presets) => presets.into_iter().find(|preset| preset.group == CHARACTER),
                    Err(e) => {
                        eprintln!("the audio groups' presets could not be read; a dummy's steps fall off as the app's other sounds do: {e:#}");
                        None
                    }
                };
                // A corpse's fall goes by the drop sounds' curve.
                let fall = match trp_data::audio::sound::drop_volume_curve(&install) {
                    Ok(curve) => Some(trp_sim::sound::landing::body_fall_volume(&curve)),
                    Err(e) => {
                        eprintln!("the drop sounds' curve could not be read; a corpse falls silent: {e:#}");
                        None
                    }
                };
                let _ = send.send((sounds, character, fall));
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
        Some(Some((mut sounds, character, fall))) => {
            banks.character = character;
            banks.fall_volume = fall;
            let (names, clips): (Vec<String>, Vec<Option<Samples>>) = std::mem::take(&mut sounds.clips).into_iter().map(|(name, clip)| (name, clip.map(Samples::of_owned))).unzip();
            let figures = sounds
                .surfaces
                .iter()
                .map(|set| (set.surface, std::array::from_fn(|way| set.banks.get(way).copied().flatten().map(|at| Figures::of(&sounds.banks[at])))))
                .collect();
            info!("the sounds of moving: {} banks of {} surfaces, {} clips read", sounds.banks.len(), sounds.surfaces.len(), clips.iter().flatten().count());
            banks.toggles = trp_data::audio::sound::TOGGLE_SOUNDS
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

pub struct StepSoundPlugin;

impl Plugin for StepSoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<BodyFell>();
        // After the inventory's moves and switches, heard on their frame.
        app.add_systems(Update, (take_banks, play, play_dummies).chain().after(crate::player::controller::step).after(crate::screens::inventory_view::Settled));
        // A corpse's fall on the frame it is made.
        app.add_systems(Update, play_falls.after(take_banks).after(crate::player::death::die).after(crate::world::dummy::fall_dummies));
    }
}
