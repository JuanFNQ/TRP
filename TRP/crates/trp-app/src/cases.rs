//! Spent cases (firing spec, "Spent cases"; the options' `SPENT CASES`). At
//! a shot of a weapon of one chamber its round's own model goes to the
//! weapon's shell port as a case (its `shell` objects on, its `full` off)
//! and rides there; at the weapon's `ShellEject` mark it is thrown from
//! where the port has carried it, with the weapon's extraction's force and
//! turn and the player's velocity, flies as the game's `BouncingObject`
//! does (`trp_sim::shells`) against the range (`Range::cast`), turning, and
//! is gone a second after it was thrown. At a bounce its sound plays
//! (`PlayShellSound`): the bank of its calibre's class and the surface met,
//! from a quarter metre over it, heard at the camera as the steps are
//! (`step_sound`), through the mixer's `CommonSounds` (0 dB with the groups
//! above it, as read: nothing added).

use std::sync::atomic::AtomicU32;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};

use bevy::math::Affine3A;
use bevy::prelude::*;
use bevy::transform::helper::TransformHelper;
use trp_data::sound::ShellSounds;
use trp_sim::recoil::Rng;
use trp_sim::shells::{Flight, ShellSettings};
use trp_sim::steps::{choose, Shuffle};

use crate::controller::Controller;
use crate::convert;
use crate::inventory_view::{Carried, HandsHeld};
use crate::loose::Loose;
use crate::range::Range;
use crate::ui_sound::{Levels, Mixer, Samples, Voice};
use crate::weapon::WeaponRig;
use crate::MainCamera;

/// What a spent case is to do, as the weapon's events say.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CaseCall {
    /// A shot: the round to the shell port (`MoveAmmoFromChamberToShellPort`).
    Shot,
    /// The `ShellEject` mark: the case thrown (`StartSpawnShell`).
    Eject,
}

/// How long a thrown case lasts (`StartAutoDestroyCountDown`'s one second).
const LASTS: f32 = 1.0;
/// The share of the player's velocity a case takes (`SpawnShell`'s call).
const PLAYER_SHARE: f32 = 0.66;
/// How far over the case its sound plays (`Vector3.up / 4`).
const SOUND_OVER: f32 = 0.25;
/// Where the source's falloff begins, as the steps' (`step_sound`).
const FULL_WITHIN: f32 = 1.0;

/// The cases: the figures they fly by (none with `SPENT CASES` off), the
/// one at the shell port, those in flight, the draws, and their sounds
/// once read.
#[derive(Resource)]
pub struct Cases {
    settings: Option<ShellSettings>,
    at_port: Option<Entity>,
    flying: Vec<Flying>,
    seed: u32,
    sounds: Option<CaseSounds>,
    coming: Option<Mutex<Receiver<ShellSounds>>>,
}

/// The cases' banks with their clips ready to mix, each bank's shuffle,
/// and the draws for the clips, volume and pitch.
struct CaseSounds {
    read: ShellSounds,
    clips: Vec<Option<Samples>>,
    shuffles: Vec<Shuffle>,
    rng: Rng,
}

/// A case in flight: its drawn model, its flight, its turn (the game's
/// axes), how long since it was thrown, and its calibre's class.
struct Flying {
    entity: Entity,
    flight: Flight,
    turn: [f32; 4],
    since: f32,
    caliber: Option<u8>,
}

impl Cases {
    /// The hard settings' figures (`EFTHardSettings.Shells`), where the
    /// options' switch is on.
    pub fn new(hard: &trp_data::hard_settings::HardSettings, on: bool) -> Self {
        let settings = on
            .then(|| -> anyhow::Result<ShellSettings> {
                Ok(ShellSettings {
                    radius: hard.float("Shells.radius")?,
                    play_mult: hard.float("Shells.playMult")?,
                    max_casts: hard.float("Shells.maxCastCount")? as i32,
                    step: hard.float("Shells.deltaTimeStep")?,
                    spread: hard.float("Shells.randomReboundSpread")?,
                    velocity_mult: hard.float("Shells.velocityMult")?,
                    velocity_rotation: hard.float("Shells.velocityRotation")?,
                    bounce: hard.float("Shells.bounceSpeedMult")?,
                    rebound_x: hard.vec2("Shells.ReboundRotationX")?,
                    rebound_y: hard.vec2("Shells.ReboundRotationY")?,
                })
            })
            .and_then(|read| {
                read.map_err(|e| eprintln!("the hard settings' figures for spent cases could not be read; none is thrown: {e:#}")).ok()
            });
        Self { settings, at_port: None, flying: Vec::new(), seed: 0x5bd1_e995, sounds: None, coming: None }
    }
}

/// Starts reading the cases' sounds away from the frame, if the options'
/// `SPENT CASES` is on.
pub fn start_reading(app: &mut App, install: trp_data::Install) {
    let mut cases = app.world_mut().resource_mut::<Cases>();
    if cases.settings.is_none() {
        return;
    }
    let (send, receive) = channel();
    std::thread::spawn(move || match trp_data::sound::shell_sounds(&install, true) {
        Ok(sounds) => {
            let _ = send.send(sounds);
        }
        Err(e) => eprintln!("the spent cases' sounds could not be read; they are silent: {e:#}"),
    });
    cases.coming = Some(Mutex::new(receive));
}

/// Takes the cases' sounds in once they have come.
fn take_sounds(cases: &mut Cases) {
    let Some(coming) = cases.coming.take() else { return };
    let got = coming.lock().ok().and_then(|receive| match receive.try_recv() {
        Ok(sounds) => Some(Some(sounds)),
        Err(std::sync::mpsc::TryRecvError::Empty) => None,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(None),
    });
    match got {
        None => cases.coming = Some(coming),
        Some(None) => {}
        Some(Some(mut read)) => {
            let clips: Vec<Option<Samples>> = std::mem::take(&mut read.clips).into_iter().map(|(_, clip)| clip.map(Samples::of_owned)).collect();
            info!("the spent cases' sounds: {} banks, {} clips read", read.banks.len(), clips.iter().flatten().count());
            let shuffles = vec![Shuffle::default(); read.banks.len()];
            cases.sounds = Some(CaseSounds { read, clips, shuffles, rng: Rng::new(0x5348_454c) });
        }
    }
}

/// A case's sound at a bounce (`PlayShellSound`, `BetterAudio.PlayAtPoint`):
/// the bank of its calibre's class and the surface met, none past the
/// bank's rolloff from the camera; its clips chosen by that distance, its
/// volume and pitch drawn (`RandomVolume`, `RandomPitch`), from a quarter
/// metre over the case, falling as one over the distance past 1 m and
/// panned at equal power (`PROVISIONAL`, as the steps').
fn play_bounce(sounds: &mut CaseSounds, caliber: u8, surface: u8, place: [f32; 3], camera: &GlobalTransform, mixer: &Mixer, overall: f32) {
    let Some(at) = trp_sim::shells::sound_surface(surface).and_then(|kind| sounds.read.bank(caliber, kind)) else { return };
    let bank = &sounds.read.banks[at];
    let case = convert::position(place);
    let distance = (case - camera.translation()).length();
    if distance > bank.rolloff {
        return;
    }
    let choice = choose(bank, &mut sounds.shuffles[at], distance, &mut sounds.rng);
    let drawn = if bank.delta_volume > 0.0 { bank.base_volume + sounds.rng.range(-bank.delta_volume, 0.0) } else { bank.base_volume };
    let to_source = case + Vec3::Y * SOUND_OVER - camera.translation();
    let heard = to_source.length();
    let falloff = if heard > FULL_WITHIN { FULL_WITHIN / heard } else { 1.0 };
    let side = if heard > 1e-4 { (to_source / heard).dot(camera.right().as_vec3()).clamp(-1.0, 1.0) } else { 0.0 };
    let angle = (side + 1.0) * std::f32::consts::FRAC_PI_4;
    // A source's volume is 0 to 1.
    let volume = drawn.clamp(0.0, 1.0) * overall * falloff;
    let pitch = Arc::new(AtomicU32::new(choice.pitch.to_bits()));
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
        mixer.0.add_to(crate::ui_sound::Group::Main, Voice::placed(samples, [angle.cos() * gain, angle.sin() * gain], pitch.clone()));
    }
}

/// A draw from 0 to 1 (the app's own: the game's draws are its engine's).
fn draw(seed: &mut u32) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    (*seed >> 8) as f32 / (1u32 << 24) as f32
}

/// `Random.Range` between two figures, either way about.
fn between(range: [f32; 2], seed: &mut u32) -> f32 {
    range[0] + (range[1] - range[0]) * draw(seed)
}

/// The round a shot of the weapon in hand fires: its chamber's, else its
/// magazine's top, else the weapon's own default.
pub(crate) fn fired_round(carried: &Carried, held: &HandsHeld, db: &trp_data::Database) -> Option<String> {
    let weapon = held.item.and_then(|id| carried.find(id))?;
    if let Some(round) = trp_sim::inventory::chambered(weapon) {
        return Some(round.kind.template.clone());
    }
    let magazine = weapon.slots.iter().zip(&weapon.kind.slots).find(|(_, slot)| slot.name == trp_sim::inventory::MAGAZINE_SLOT).and_then(|(part, _)| part.as_ref());
    if let Some(kind) = magazine.and_then(|magazine| trp_sim::inventory::magazine_round_at(magazine, 0)) {
        return Some(kind.template.clone());
    }
    db.items.get(&weapon.kind.template).and_then(|item| item.props.get("defAmmo")).and_then(|ammo| ammo.as_str()).map(str::to_owned)
}

/// The quaternion product, `b` first.
fn mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    trp_sim::math::quat_mul(a, b)
}

/// A way turned by a quaternion.
fn rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let r = Quat::from_xyzw(q[0], q[1], q[2], q[3]) * Vec3::from(v);
    [r.x, r.y, r.z]
}

/// The cases' frame: the calls taken, the case at the port made or thrown,
/// those in flight moved on, turned, and taken away when their second is up.
#[allow(clippy::too_many_arguments)]
pub fn fly_cases(
    mut commands: Commands,
    mut cases: ResMut<Cases>,
    mut controller: ResMut<Controller>,
    carried: Res<Carried>,
    held: Res<HandsHeld>,
    game: Res<crate::GameData>,
    mut loose: ResMut<Loose>,
    weapon: Option<Res<WeaponRig>>,
    range: Res<Range>,
    time: Res<Time>,
    mut transforms: ParamSet<(TransformHelper, Query<&mut Transform>)>,
    mixer: Option<Res<Mixer>>,
    levels: Option<Res<Levels>>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
) {
    let cases = &mut *cases;
    let calls = std::mem::take(&mut controller.case_calls);

    let Some(settings) = cases.settings else { return };
    take_sounds(cases);
    let port = weapon.as_ref().and_then(|rig| rig.shell_port);
    // The round's case's model asked for ahead of the first shot (the
    // game's pool has it made).
    if port.is_some() && (held.is_changed() || carried.is_changed()) {
        if let Some(round) = fired_round(&carried, &held, &game.db) {
            loose.model_for(&format!("{}{round}", crate::loose::CASE_MODEL));
        }
    }
    for call in calls {
        match call {
            // The round to the shell port, a quarter turn about x
            // (`ParentShellToTransform`), a case.
            CaseCall::Shot => {
                // One left at the port goes (with its rig, if that is gone).
                if let Some(old) = cases.at_port.take() {
                    commands.entity(old).try_despawn();
                }
                let (Some(port), Some(round)) = (port, fired_round(&carried, &held, &game.db)) else { continue };
                let Some(ready) = loose.model_for(&format!("{}{round}", crate::loose::CASE_MODEL)) else { continue };
                let turn = convert::rotation(trp_sim::math::quat_euler([90.0, 0.0, 0.0]));
                let case = spawn_case(&mut commands, &ready, Transform::from_rotation(turn));
                commands.entity(port.entity).add_child(case);
                cases.at_port = Some(case);
            }
            // Thrown from where the port has carried it, with the weapon's
            // force and turn (`SpawnExtractingCartridge`).
            CaseCall::Eject => {
                let (Some(case), Some(port), Some(rig)) = (cases.at_port.take(), port, weapon.as_ref()) else { continue };
                let helper = transforms.p0();
                let unsqueezed = |entity: Entity| -> Option<([f32; 3], [f32; 4])> {
                    let root = helper.compute_global_transform(rig.root).ok()?;
                    let (_, turn, at) = root.to_scale_rotation_translation();
                    let now = helper.compute_global_transform(entity).ok()?;
                    let (_, turn_now, at_now) = (Affine3A::from_rotation_translation(turn, at) * root.affine().inverse() * now.affine()).to_scale_rotation_translation();
                    Some(([at_now.x, at_now.y, -at_now.z], convert::rotation_back(turn_now)))
                };
                let pose = unsqueezed(port.entity);
                let local = transforms.p1().get(port.entity).map(|transform| *transform);
                let (Some((at, port_turn)), Ok(local)) = (pose, local) else {
                    commands.entity(case).try_despawn();
                    continue;
                };
                commands.entity(case).try_despawn();
                let case_turn = mul(port_turn, trp_sim::math::quat_euler([90.0, 0.0, 0.0]));
                let moved = [local.translation.x - port.rest[0], local.translation.y - port.rest[1], -local.translation.z - port.rest[2]];
                let e = port.extraction;
                let seed = &mut cases.seed;
                let force: [f32; 3] = std::array::from_fn(|k| moved[k] * e.multiplier + between(e.force[k], seed));
                let thrown = rotate(case_turn, force);
                let player = controller.movement.velocity;
                let velocity: [f32; 3] = std::array::from_fn(|k| thrown[k] + player[k] * PLAYER_SHARE);
                let spin: [f32; 3] = std::array::from_fn(|k| between(e.turn[k], seed));
                let forward = rotate(port_turn, [0.0, 0.0, 1.0]);
                let Some(round) = fired_round(&carried, &held, &game.db) else { continue };
                let Some(ready) = loose.model_for(&format!("{}{round}", crate::loose::CASE_MODEL)) else { continue };
                let cast = |from: [f32; 3], way: [f32; 3], reach: f32| range.cast(from, way, reach);
                let mut random = || draw(seed);
                let flight = Flight::new(at, velocity, spin, forward, settings, &cast, &mut random);
                let entity = spawn_case(&mut commands, &ready, Transform { translation: convert::position(at), rotation: convert::rotation(case_turn), ..default() });
                cases.flying.push(Flying { entity, flight, turn: case_turn, since: 0.0, caliber: ready.caliber });
            }
        }
    }
    // Those in flight.
    let dt = time.delta_secs();
    let cast = |from: [f32; 3], way: [f32; 3], reach: f32| range.cast(from, way, reach);
    let seed = &mut cases.seed;
    let hearing = mixer.as_deref().zip(camera.single().ok());
    let overall = levels.as_deref().map_or(1.0, Levels::overall_gain);
    let sounds = &mut cases.sounds;
    cases.flying.retain_mut(|case| {
        case.since += dt;
        if case.since >= LASTS {
            commands.entity(case.entity).despawn();
            return false;
        }
        let mut random = || draw(seed);
        let struck = case.flight.frame(dt, &cast, &mut random);
        if let (Some(surface), Some(caliber), Some(sounds), Some((mixer, camera))) = (struck, case.caliber, sounds.as_mut(), hearing) {
            play_bounce(sounds, caliber, surface, case.flight.place(), camera, mixer, overall);
        }
        // Its turn each frame (`Shell.Update`: its turn times a hundred
        // times the frame's time, in degrees, about its own axes).
        let step = trp_sim::math::quat_euler(case.flight.turn.map(|t| t * dt * 100.0));
        case.turn = mul(case.turn, step);
        if let Ok(mut transform) = transforms.p1().get_mut(case.entity) {
            transform.translation = convert::position(case.flight.place());
            transform.rotation = convert::rotation(case.turn);
        }
        true
    });
}

/// A case's drawn model: the round's own, its `shell` objects on and its
/// `full` ones off (`AmmoPoolObject.SetUsed`), at that place.
fn spawn_case(commands: &mut Commands, ready: &crate::loose::Ready, at: Transform) -> Entity {
    let root = commands.spawn((Name::new("Spent case"), at, Visibility::Inherited)).id();
    let mut entities = Vec::with_capacity(ready.nodes.len());
    for (index, (parent, own, on)) in ready.nodes.iter().enumerate() {
        let name = ready.names.get(index).map(String::as_str).unwrap_or_default();
        let shown = if name.contains("full") {
            false
        } else {
            name.contains("shell") || *on
        };
        let entity = commands.spawn((*own, if shown { Visibility::Inherited } else { Visibility::Hidden })).id();
        commands.entity(parent.map_or(root, |p| entities[p])).add_child(entity);
        entities.push(entity);
    }
    for (node, mesh, material) in &ready.drawn {
        let piece = commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), Transform::default())).id();
        commands.entity(entities[*node]).add_child(piece);
    }
    root
}
