//! The rig in the hands as it is drawn.
//!
//! In play the rig is posed by its own animator, which the simulation
//! runs as a state machine (`trp_sim::weapon_animator`): `pose_rig` writes
//! that pose, blended between the simulation's frames, onto the rig's
//! bones. The clip player below is for looking at one clip by its name
//! (`--play`, `--at`), which is the only time it plays.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use bevy::prelude::*;
use trp_data::unity::anim::{euler_to_quat, Clip, Property};
use trp_data::unity::ObjRef;

use crate::weapon::WeaponRig;
use crate::{convert, GameData};

/// Clips of the spawned weapon by the name its controller uses.
#[derive(Resource, Default)]
pub struct ClipLibrary {
    pub clips: HashMap<String, ObjRef>,
}

struct Target {
    entity: Entity,
    property: Property,
    curve: usize,
}

/// A clip read from the install and bound to the rig's bones.
struct Loaded {
    clip: Clip,
    targets: Vec<Target>,
}

struct Playing {
    name: String,
    data: Arc<Loaded>,
    time: f32,
    speed: f32,
}

/// What was showing before, fading out under the clip that replaced it.
struct Fade {
    from: Playing,
    elapsed: f32,
    duration: f32,
}

#[derive(Resource)]
pub struct ClipPlayer {
    /// Clip that plays whenever nothing else is queued.
    pub idle: String,
    pub queue: VecDeque<String>,
    /// Freeze playback at this time. Used for still captures.
    pub hold_at: Option<f32>,
    /// Whether clips are played by name in place of the rig's animator
    /// (`--play`, `--at`).
    pub by_name: bool,
    playing: Option<Playing>,
    fade: Option<Fade>,
    /// Clips already read, so that entering a state does not stall a frame.
    loaded: HashMap<String, Arc<Loaded>>,
    values: Vec<f32>,
    sampled: bool,
}

impl ClipPlayer {
    pub fn new(idle: &str) -> Self {
        Self {
            idle: idle.to_owned(),
            queue: VecDeque::new(),
            hold_at: None,
            by_name: false,
            playing: None,
            fade: None,
            loaded: HashMap::new(),
            values: Vec::new(),
            sampled: false,
        }
    }

    /// Whether a clip wrote the rig's bones this frame.
    pub fn sampled_this_frame(&self) -> bool {
        self.sampled
    }

    /// Name and time of the clip on screen.
    pub fn status(&self) -> Option<(&str, f32, f32)> {
        self.playing.as_ref().map(|p| (p.name.as_str(), p.time, p.data.clip.duration()))
    }

    /// How far the clip on screen has blended in over the one before it, 0 to 1.
    pub fn blend(&self) -> f32 {
        self.fade.as_ref().map_or(1.0, |f| (f.elapsed / f.duration).clamp(0.0, 1.0))
    }
}

pub fn play_clips(
    time: Res<Time>,
    game: Res<GameData>,
    library: Option<Res<ClipLibrary>>,
    rig: Option<Res<WeaponRig>>,
    mut player: ResMut<ClipPlayer>,
    mut transforms: Query<&mut Transform>,
) {
    player.sampled = false;
    if !player.by_name {
        return;
    }
    let (Some(library), Some(rig)) = (library, rig) else { return };
    let player = &mut *player;

    let finished = player.playing.as_ref().is_some_and(|p| p.time >= p.data.clip.duration());
    let holding = player.hold_at.is_some() && player.playing.is_some();
    if player.playing.is_none() || (finished && !holding) {
        let was_idle = player.playing.as_ref().is_some_and(|p| p.name == player.idle);
        match player.queue.pop_front() {
            Some(name) => player.playing = start(&game, &library, &rig, &mut player.loaded, &name),
            // The idle clip stays on: one that loops begins again, and one
            // that does not rests on its last frame, as the animator
            // leaves a state whose clip has run out. Anything else hands
            // back to it.
            None if was_idle => {
                let idle = player.playing.as_mut().unwrap();
                if idle.data.clip.looping {
                    idle.time = 0.0;
                }
            }
            None => {
                let idle = player.idle.clone();
                player.playing = start(&game, &library, &rig, &mut player.loaded, &idle);
            }
        }
    }
    let dt = time.delta_secs();
    let Some(playing) = &mut player.playing else { return };
    playing.time = match player.hold_at {
        Some(at) => at.min(playing.data.clip.duration()),
        None => (playing.time + dt * playing.speed).min(playing.data.clip.duration()),
    };

    // What is fading out is written first and in full; the clip on screen
    // is then laid over it by as much as the blend has run.
    let mut weight = 1.0;
    if let Some(fade) = &mut player.fade {
        fade.elapsed += dt;
        if fade.elapsed >= fade.duration {
            player.fade = None;
        } else {
            let from = &mut fade.from;
            from.time = (from.time + dt * from.speed).min(from.data.clip.duration());
            from.data.clip.sample(from.time, &mut player.values);
            write(&from.data.targets, &player.values, 1.0, &mut transforms);
            weight = fade.elapsed / fade.duration;
        }
    }
    playing.data.clip.sample(playing.time, &mut player.values);
    write(&playing.data.targets, &player.values, weight, &mut transforms);
    player.sampled = true;
}

/// Puts the rig in the hands as its animator has it at the moment drawn
/// (`Controller::rig_shown`): every bone but the rig's root, which hangs
/// in the view's frame, and the bone the procedural motion moves, which
/// the weapon motion places. A bone that a clip scales takes its scale as
/// the animator last had it (an item used is drawn by its bone scaled up
/// from nought); the rest keep the prefab's own.
pub fn pose_rig(
    controller: Res<crate::controller::Controller>,
    rig: Option<Res<WeaponRig>>,
    hands: Option<Res<crate::weapon::HandsRig>>,
    mut player: ResMut<ClipPlayer>,
    mut transforms: Query<&mut Transform>,
) {
    if player.by_name {
        return;
    }
    let (Some(rig), Some(pose)) = (rig, controller.rig_shown()) else { return };
    // A pose of the other rig, for the frame in which the two are changed.
    if pose.len() != rig.bones.len() {
        return;
    }
    for (entity, (at, turn)) in rig.bones.iter().zip(&pose).skip(1) {
        if Some(*entity) == rig.weapon_root_anim {
            continue;
        }
        if let Ok(mut transform) = transforms.get_mut(*entity) {
            transform.translation = convert::position(*at);
            transform.rotation = convert::rotation(*turn);
        }
    }
    for (bone, scale) in hands.iter().flat_map(|hands| hands.animator()).flat_map(|animator| animator.scales()) {
        let Some(entity) = rig.bones.get(bone).filter(|entity| bone > 0 && Some(**entity) != rig.weapon_root_anim) else { continue };
        if let Ok(mut transform) = transforms.get_mut(*entity) {
            let scale = Vec3::from(scale);
            if transform.scale != scale {
                transform.scale = scale;
            }
        }
    }
    player.sampled = true;
}

/// Moves each bone toward the clip's value by `weight`.
fn write(targets: &[Target], v: &[f32], weight: f32, transforms: &mut Query<&mut Transform>) {
    for target in targets {
        let Ok(mut transform) = transforms.get_mut(target.entity) else { continue };
        let c = target.curve;
        match target.property {
            Property::Position => {
                let to = convert::position([v[c], v[c + 1], v[c + 2]]);
                transform.translation = transform.translation.lerp(to, weight);
            }
            Property::Scale => {
                let to = Vec3::new(v[c], v[c + 1], v[c + 2]);
                transform.scale = transform.scale.lerp(to, weight);
            }
            Property::Rotation | Property::Euler => {
                let to = convert::rotation(match target.property {
                    Property::Rotation => [v[c], v[c + 1], v[c + 2], v[c + 3]],
                    _ => euler_to_quat([v[c], v[c + 1], v[c + 2]]),
                });
                transform.rotation = if weight >= 1.0 { to } else { transform.rotation.slerp(to, weight) };
            }
        }
    }
}

fn start(
    game: &GameData,
    library: &ClipLibrary,
    rig: &WeaponRig,
    loaded: &mut HashMap<String, Arc<Loaded>>,
    name: &str,
) -> Option<Playing> {
    let data = match loaded.get(name) {
        Some(data) => data.clone(),
        None => {
            let Some(&obj) = library.clips.get(name) else {
                warn!("no clip named {name}");
                return None;
            };
            let clip = match Clip::read(&game.unity, obj) {
                Ok(clip) => clip,
                Err(e) => {
                    warn!("clip {name}: {e:#}");
                    return None;
                }
            };
            // Tracks for bones this build of the weapon does not have are dropped.
            let targets = clip
                .tracks
                .iter()
                .filter_map(|t| Some(Target { entity: *rig.by_hash.get(&t.path)?, property: t.property, curve: t.curve }))
                .collect();
            let data = Arc::new(Loaded { clip, targets });
            loaded.insert(name.to_owned(), data.clone());
            data
        }
    };
    Some(Playing { name: name.to_owned(), data, time: 0.0, speed: 1.0 })
}
