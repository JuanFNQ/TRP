//! A weapon heard as it lands (sound spec, "A weapon landing"; the options'
//! `WORLD SOUNDS`): a weapon put down from the inventory, and the weapon a
//! death lets go (a dummy's or the player's), each reporting what its body
//! begins to touch (`trp_physics::World::report_touches`); the rule says
//! which touches are heard and how hard (`trp_sim::landing`), and here each
//! plays its surface's bank for its kind of weapon (`ItemDropSounds`, read
//! from the install as the run begins) at the volume the install's curve
//! gives the energy, where the weapon is (`BetterAudio.PlayDropItem`:
//! `PlayAtPoint`, the bank's own source kind, `Collisions`, which goes to
//! the world's group at 0 dB).

use std::sync::mpsc::{channel, Receiver};
use std::sync::Mutex;

use bevy::prelude::*;
use trp_data::audio_preset::AudioGroupPreset;
use trp_data::sound::DropSounds;
use trp_sim::recoil::Rng;
use trp_sim::steps::Shuffle;

use crate::convert;
use crate::ui_sound::{Levels, Mixer, Samples};
use crate::MainCamera;

/// A weapon's touch the rule heard: where the weapon is (the game's axes),
/// its kind of landing sound (`trp_data::sound::drop_kind`), what it met's
/// surface (`ESurfaceSound`) and the energy.
#[derive(Message, Clone, Copy, Debug)]
pub struct Landed {
    pub at: [f32; 3],
    pub kind: u8,
    pub surface: u8,
    pub energy: f32,
}

/// What a body that reports its touches began to touch since it was last
/// asked, put to its landing rule at the game's time `now`: a still thing
/// of the range by the range's surface where the touch is (a short line
/// through the point of contact along its normal, as the steps find their
/// surface; concrete where the line finds nothing, the surface of a thing
/// that has none said), a body by `on_body`'s surface (a corpse's body
/// part's; none for what has no surface). The touches heard.
/// `PROVISIONAL`: the range's colliders carry no `BaseBallistic` of their
/// own, so every still thing counts as having a surface, the range's.
pub fn hear(world: &mut trp_physics::World, body: trp_physics::Body, landing: &mut trp_sim::landing::Landing, now: f32, range: &crate::range::Range, on_body: impl Fn(trp_physics::Body) -> Option<u8>) -> Vec<Landed> {
    use trp_sim::movement::World as _;
    let touches = world.take_touches();
    if touches.is_empty() || landing.kind == 0 {
        return Vec::new();
    }
    let gravity = world.settings().gravity;
    let gravity = (gravity[0] * gravity[0] + gravity[1] * gravity[1] + gravity[2] * gravity[2]).sqrt();
    let mut heard = Vec::new();
    for touch in touches.into_iter().filter(|touch| touch.body == body) {
        let surface = match touch.other {
            trp_physics::Met::Still(_) => Some(touch.contact.map_or(trp_sim::steps::CONCRETE, |(point, normal)| {
                let out = |by: f32| std::array::from_fn(|k| point[k] + normal[k] * by);
                range.surface(out(SURFACE_REACH), out(-SURFACE_REACH)).unwrap_or(trp_sim::steps::CONCRETE)
            })),
            trp_physics::Met::Body(other) => on_body(other),
        };
        let (at, _) = world.pose(body);
        let (velocity, _) = world.velocity(body);
        let speed = (velocity[0] * velocity[0] + velocity[1] * velocity[1] + velocity[2] * velocity[2]).sqrt();
        if let (Some(energy), Some(surface)) = (landing.touched(now, surface, at[1], speed, gravity), surface) {
            heard.push(Landed { at, kind: landing.kind, surface, energy });
        }
    }
    heard
}

/// How far either side of a point of contact the line that finds its
/// surface reaches (this work's).
const SURFACE_REACH: f32 = 0.05;

/// The banks once read, their clips ready to mix, each bank's shuffle, the
/// draws, and what brings them.
#[derive(Resource)]
pub struct LandingSounds {
    read: Option<DropSounds>,
    /// The banks' source group's preset (`Collisions`), whose rolloff the
    /// sounds fall by; the app's own falloff where it could not be read.
    preset: Option<AudioGroupPreset>,
    clips: Vec<Option<Samples>>,
    shuffles: Vec<Shuffle>,
    rng: Rng,
    coming: Option<Mutex<Receiver<(DropSounds, Option<AudioGroupPreset>)>>>,
    logged: usize,
}

/// The banks' source group (`AudioSourceGroupType.Collisions`, every bank's
/// `SourceType` as read).
const COLLISIONS: i32 = 5;

/// Starts reading the weapons' landing sounds away from the frame, if the
/// options' `WORLD SOUNDS` is on; off, nothing is read and nothing plays.
/// The message is had either way, for the bodies to tell.
pub fn start_reading(app: &mut App, install: trp_data::Install, on: bool) {
    app.add_message::<Landed>();
    if !on {
        return;
    }
    let (send, receive) = channel();
    std::thread::spawn(move || match trp_data::sound::drop_sounds(&install, true) {
        Ok(sounds) => {
            let preset = match trp_data::audio_preset::audio_group_presets(&install) {
                Ok(presets) => presets.into_iter().find(|preset| preset.group == COLLISIONS),
                Err(e) => {
                    eprintln!("the audio groups' presets could not be read; a landing falls off as the app's other sounds do: {e:#}");
                    None
                }
            };
            let _ = send.send((sounds, preset));
        }
        Err(e) => eprintln!("the weapons' landing sounds could not be read; they are silent: {e:#}"),
    });
    app.insert_resource(LandingSounds { read: None, preset: None, clips: Vec::new(), shuffles: Vec::new(), rng: Rng::new(0x4452_4f50), coming: Some(Mutex::new(receive)), logged: 0 });
    app.add_systems(Update, play_landings.run_if(resource_exists::<LandingSounds>));
}

/// Plays the touches heard this frame: each its surface's bank for its
/// kind (the default surface's where its own has no set), at the curve's
/// volume of its energy, rounded to two places.
pub fn play_landings(mut landed: MessageReader<Landed>, mut sounds: ResMut<LandingSounds>, mixer: Option<Res<Mixer>>, levels: Option<Res<Levels>>, camera: Query<&GlobalTransform, With<MainCamera>>) {
    let sounds = &mut *sounds;
    if let Some(coming) = sounds.coming.take() {
        match coming.lock().ok().map(|receive| receive.try_recv()) {
            Some(Ok((mut read, preset))) => {
                sounds.clips = std::mem::take(&mut read.clips).into_iter().map(|(_, clip)| clip.map(Samples::of_owned)).collect();
                sounds.shuffles = vec![Shuffle::default(); read.banks.len()];
                info!("the weapons' landing sounds: {} surfaces, {} banks, {} clips read; {}", read.surfaces.len(), read.banks.len(), sounds.clips.iter().flatten().count(), if preset.is_some() { "the Collisions group's rolloff" } else { "no group rolloff" });
                sounds.read = Some(read);
                sounds.preset = preset;
            }
            Some(Err(std::sync::mpsc::TryRecvError::Empty)) => sounds.coming = Some(coming),
            _ => {}
        }
    }
    let (Some(mixer), Ok(camera), Some(read)) = (mixer, camera.single(), sounds.read.as_ref()) else {
        landed.clear();
        return;
    };
    let overall = levels.map_or(1.0, |levels| levels.overall_gain());
    for touch in landed.read() {
        let Some(at) = read.bank(touch.surface, touch.kind) else { continue };
        let volume = trp_sim::landing::volume(read.volume.evaluate(touch.energy));
        let bank = &read.banks[at];
        let played = crate::impact_sound::play_at_point(&mixer, bank, &sounds.clips, &mut sounds.shuffles[at], &mut sounds.rng, convert::position(touch.at), camera, volume, overall, sounds.preset.as_ref());
        if sounds.logged < 32 {
            sounds.logged += 1;
            match played {
                Some((distance, drawn)) => info!("a weapon landed ({}): {:.1} J, volume {volume:.2} (at {:.2}), {distance:.1} m off", bank.name, touch.energy, volume * drawn),
                None => info!("a weapon landed ({}): {:.1} J, out of hearing", bank.name, touch.energy),
            }
        }
    }
}
