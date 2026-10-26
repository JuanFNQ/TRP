//! The things' own sounds (interactions spec, "A thing's sounds"; the
//! options' `WORLD SOUNDS`): what a door, a container or a trunk of the
//! range plays as it swings, shuts or is kicked, from the clips its Customs
//! prefab names (`trp_data::thing_sound`, read from the build's scenes as
//! the run begins). The simulation says which sound and on which frame
//! (`Thing::take_sounds`); here each is given its clip (one of the list at
//! random, `Random.Range(0, n)`), its volume (drawn from the range the
//! game gives it: a door's and a trunk's opening 0.6 to 0.75, a
//! container's 0.5 to 0.55, a shutting 0.2 to 0.3, a kick's breach and hit
//! 1), its place (the thing's own, at its aiming point's height; a door's
//! moved half a metre towards the player's capsule centre) and its
//! hearing: nothing beyond the list's rolloff; plain stereo within 3 m of
//! the capsule centre (`forceStereo`); beyond, placed as the bullets' hits
//! are, falling as one over the distance past 1 m and panned at equal
//! power (`PROVISIONAL`). All through the world's group (`CommonSounds`:
//! 0 dB with the groups above it, as read), at the overall volume.

use std::collections::HashMap;
use std::sync::atomic::AtomicU32;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use trp_data::thing_sound::{ThingKind, ThingSounds, SCENES};
use trp_sim::recoil::Rng;
use trp_sim::things::{turn, Kind, ThingSound};

use crate::controller::Controller;
use crate::convert;
use crate::range::Range;
use crate::step_sound::SoundLog;
use crate::ui_sound::{Group, Levels, Mixer, Samples, Voice};
use crate::MainCamera;

/// Within this of the player's capsule centre a thing's sound is plain
/// stereo, not placed (`PlaySoundAtPoint`: the distance squared over 9).
const STEREO_WITHIN: f32 = 3.0;
/// Where a placed source's falloff begins (as the bullets' hits').
const FULL_WITHIN: f32 = 1.0;
/// The volume ranges (`OpenSoundVolumeRange`: the base class's and
/// `LootableContainer`'s; `ShutSoundVolumeRange`).
const DOOR_OPENING: [f32; 2] = [0.6, 0.75];
const CONTAINER_OPENING: [f32; 2] = [0.5, 0.55];
const SHUTTING: [f32; 2] = [0.2, 0.3];

/// The things' clip sets once read, their clips ready to mix, the draws,
/// and what brings them.
#[derive(Resource)]
pub struct ThingSoundBanks {
    read: Option<ThingSounds>,
    clips: Vec<Option<Samples>>,
    rng: Rng,
    coming: Option<Mutex<Receiver<ThingSounds>>>,
    /// The prefab each of the range's things stands for, by its place in
    /// the range, once the sets have come and the shut shifts are set.
    sets: HashMap<usize, usize>,
    placed: bool,
    logged: usize,
}

/// Starts reading the scenes' things away from the frame, if the options'
/// `WORLD SOUNDS` is on; off, nothing is read and nothing plays.
pub fn start_reading(app: &mut App, install: trp_data::Install, on: bool) {
    if !on {
        return;
    }
    let (send, receive) = channel();
    std::thread::spawn(move || match trp_data::thing_sound::thing_sounds(&install, &SCENES, true) {
        Ok(sounds) => {
            let _ = send.send(sounds);
        }
        Err(e) => eprintln!("the things' sounds could not be read; they are silent: {e:#}"),
    });
    app.insert_resource(ThingSoundBanks { read: None, clips: Vec::new(), rng: Rng::new(0x5448_494e), coming: Some(Mutex::new(receive)), sets: HashMap::new(), placed: false, logged: 0 });
}

/// The Customs prefab a thing of the range stands for, by the thing's
/// name (`range.rs`): the metal dorm door for the doors; the wooden crate
/// and the supply crate theirs; the dead Scav a corpse's; a drawer, and
/// the cabinets' drawers and dressers, a card file's; a weapon case the
/// weapon box's; a magazine box and the cabinets' boxes the ammunition
/// box's; the trunk a gun safe's door (the one `Trunk` the scenes hold:
/// `PROVISIONAL`, the car's own prefab is not among them); anything else
/// the wooden crate's.
fn prefab_of(name: &str, kind: Kind) -> &'static str {
    match (name, kind) {
        (_, Kind::Door) => "Outside_Door_Metal_17_R_210-100_Door",
        (_, Kind::Trunk) => "gun_safe_DoorL",
        ("wooden crate", _) => "scontainer_wood_CAP",
        ("technical supply crate", _) => "container_supplyBox_opened_Cap",
        ("dead Scav", _) => "corpse_scav_03_LOOTABLE",
        (name, _) if name == "drawer" || name.ends_with(" drawer") || name.ends_with(" dresser") => "card_file_box_01",
        (name, _) if name.ends_with(" case") => "Weapon_box_01_cap",
        (name, _) if name.ends_with(" box") => "scontainer_ammo_wood_01_Cap",
        _ => "scontainer_wood_CAP",
    }
}

/// Takes the sets in once they have come, gives each thing of the range
/// its prefab's set and its shut shift, and plays what the things sounded
/// this frame. Runs after the range has stepped its things.
pub fn play_things(
    mut banks: ResMut<ThingSoundBanks>,
    mut range: ResMut<Range>,
    controller: Res<Controller>,
    mixer: Option<Res<Mixer>>,
    levels: Option<Res<Levels>>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    log: Option<Res<SoundLog>>,
) {
    let banks = &mut *banks;
    if let Some(coming) = banks.coming.take() {
        match coming.lock().ok().map(|receive| receive.try_recv()) {
            Some(Ok(mut read)) => {
                banks.clips = std::mem::take(&mut read.clips).into_iter().map(|(_, clip)| clip.map(Samples::of_owned)).collect();
                info!("the things' sounds: {} things of {} scenes, {} clips read", read.sets.len(), SCENES.len(), banks.clips.iter().flatten().count());
                banks.read = Some(read);
            }
            Some(Err(std::sync::mpsc::TryRecvError::Empty)) => banks.coming = Some(coming),
            _ => {}
        }
    }
    // The sounds a thing made before its set came are let go: the things
    // stand still as a run begins.
    let Some(read) = banks.read.as_ref() else {
        for placed in &mut range.things {
            placed.thing.take_sounds();
        }
        return;
    };
    if !banks.placed {
        banks.placed = true;
        for (index, placed) in range.things.iter_mut().enumerate() {
            let prefab = prefab_of(placed.name, placed.thing.kind);
            let Some(at) = read.sets.iter().position(|set| set.name == prefab) else {
                warn!("the {}: no thing named {prefab} in the scenes read; silent", placed.name);
                continue;
            };
            placed.thing.shut_shift = read.sets[at].shut_shift;
            banks.sets.insert(index, at);
        }
    }
    let (Some(mixer), Ok(camera)) = (mixer, camera.single()) else {
        for placed in &mut range.things {
            placed.thing.take_sounds();
        }
        return;
    };
    let overall = levels.map_or(1.0, |levels| levels.overall_gain());
    let listener = camera.translation();
    let [px, py, pz] = controller.movement.position;
    let centre = convert::position([px, py + controller.movement.capsule_figures()[1], pz]);
    for index in 0..range.things.len() {
        let sounds = range.things[index].thing.take_sounds();
        if sounds.is_empty() {
            continue;
        }
        let Some(&at) = banks.sets.get(&index) else { continue };
        let set = &read.sets[at];
        let placed = &range.things[index];
        let source = convert::position(sound_place(placed, controller.movement.position, controller.movement.capsule_figures()[1]));
        for sound in sounds {
            // Which lists, at what volume, with what rolloff.
            let opening = if set.kind == ThingKind::Container { CONTAINER_OPENING } else { DOOR_OPENING };
            let plays: Vec<(&[usize], f32, f32)> = match sound {
                ThingSound::Swing { opens, squeaks } => {
                    let volume = banks.rng.range(opening[0], opening[1]);
                    let mut plays = Vec::new();
                    if opens {
                        plays.push((set.open.as_slice(), volume, set.rolloffs[0]));
                    }
                    if squeaks {
                        plays.push((set.squeak.as_slice(), volume, set.rolloffs[1]));
                    }
                    plays
                }
                ThingSound::Shut => vec![(set.shut.as_slice(), banks.rng.range(SHUTTING[0], SHUTTING[1]), set.rolloffs[2])],
                ThingSound::Breach => vec![(set.breach.as_slice(), 1.0, set.breach_rolloff)],
                ThingSound::Hit => vec![(set.hit.as_slice(), 1.0, set.hit_rolloff)],
            };
            for (list, volume, rolloff) in plays {
                if list.is_empty() {
                    continue;
                }
                let drawn = (banks.rng.range(0.0, list.len() as f32) as usize).min(list.len() - 1);
                let clip = list[drawn];
                let Some(samples) = banks.clips.get(clip).and_then(Option::as_ref) else { continue };
                let heard = (source - listener).length();
                if heard >= rolloff {
                    continue;
                }
                let gain = volume.clamp(0.0, 1.0) * overall;
                let near = (source - centre).length_squared() <= STEREO_WITHIN * STEREO_WITHIN;
                let said = if near {
                    mixer.0.add_to(Group::Main, Voice::whole(samples, gain));
                    "stereo".to_owned()
                } else {
                    let to_source = source - listener;
                    let falloff = if heard > FULL_WITHIN { FULL_WITHIN / heard } else { 1.0 };
                    let side = if heard > 1e-4 { (to_source / heard).dot(camera.right().as_vec3()).clamp(-1.0, 1.0) } else { 0.0 };
                    let angle = (side + 1.0) * std::f32::consts::FRAC_PI_4;
                    let pitch = Arc::new(AtomicU32::new(1.0f32.to_bits()));
                    mixer.0.add_to(Group::Main, Voice::placed(samples, [angle.cos() * gain * falloff, angle.sin() * gain * falloff], pitch));
                    format!("placed, falloff {falloff:.3}")
                };
                if banks.logged < 32 {
                    banks.logged += 1;
                    info!("the {}: {:?} {} at {volume:.3}, {heard:.1} m off, {said}", placed.name, sound, read.clip_name(clip));
                }
                if let Some(log) = &log {
                    use std::io::Write;
                    if let Ok(mut out) = log.0.lock() {
                        let _ = writeln!(out, "{:9.3} thing        {:28} {:?} volume {volume:.4} distance {heard:.3} {said} [{}]", controller.movement.frame_clock(), placed.name, sound, read.clip_name(clip));
                        let _ = out.flush();
                    }
                }
            }
        }
    }
}

/// Where a thing sounds from (`GetSoundPos`), in the simulation's axes:
/// its own place at the height of its aiming point; a door's moved half a
/// metre from that point towards the player's capsule centre.
fn sound_place(placed: &crate::range::Placed, feet: [f32; 3], centre_up: f32) -> [f32; 3] {
    let thing = &placed.thing;
    let [x, _, z] = placed.place;
    let mut at = [x, placed.place[1] + thing.view_target[1], z];
    if let (Kind::Door, Some(hinge)) = (thing.kind, thing.hinge) {
        let along = turn(hinge.forward, hinge.axis, thing.angle());
        let target: [f32; 3] = std::array::from_fn(|k| hinge.at[k] + along[k] * thing.view_target[0] + if k == 1 { thing.view_target[1] } else { 0.0 });
        let centre = [feet[0], feet[1] + centre_up, feet[2]];
        let to: [f32; 3] = std::array::from_fn(|k| centre[k] - target[k]);
        let length = (to[0] * to[0] + to[1] * to[1] + to[2] * to[2]).sqrt();
        if length > 1e-4 {
            for k in 0..3 {
                at[k] += to[k] / length * 0.5;
            }
        }
    }
    at
}
