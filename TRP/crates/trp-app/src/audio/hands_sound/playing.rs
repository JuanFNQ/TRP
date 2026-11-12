//! The hands' sounds played each frame.

use super::*;

/// Plays the marks the hands passed this frame.
#[allow(clippy::too_many_arguments)]
pub(super) fn play(
    mut controller: ResMut<Controller>,
    hands: Option<Res<HandsRig>>,
    mixer: Option<Res<Mixer>>,
    levels: Option<Res<Levels>>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    mut draws: ResMut<Draws>,
    log: Option<Res<crate::audio::step_sound::SoundLog>>,
    time: Res<Time>,
    mut aiming: ResMut<AimingBanks>,
    (rig, range, places): (Option<Res<crate::weapons::weapon::WeaponRig>>, Option<Res<crate::world::range::Range>>, Query<&GlobalTransform>),
) {
    if controller.hands_sounds.is_empty() && controller.launcher_sounds.is_empty() && controller.aims_up == 0 && controller.mount_sounds == 0 && controller.bipod_grounds == 0 && controller.bipod_marks.is_empty() {
        return;
    }
    let marks = std::mem::take(&mut controller.hands_sounds);
    let launcher_marks = std::mem::take(&mut controller.launcher_sounds);
    let aims = std::mem::take(&mut controller.aims_up);
    let mounts = std::mem::take(&mut controller.mount_sounds);
    let grounds = std::mem::take(&mut controller.bipod_grounds);
    let bipod_marks = std::mem::take(&mut controller.bipod_marks);
    let (Some(hands), Some(mixer)) = (hands, mixer) else { return };
    let Some(sounds) = hands.sounds.as_ref() else { return };
    let overall = levels.map_or(1.0, |levels| levels.overall_gain());
    let own_eyes = controller.movement.body.point_of_view() == trp_sim::PointOfView::FirstPerson;
    // The bipod's own sounds (`BipodAudioController`): opened or folded by
    // the hands' marks; on the ground the surface a line from the bipod
    // toward the mount's point meets within 1 m (`PlayMountSound`: concrete
    // where it meets nothing). Each one of its set's clips at random, at the
    // set's volume, at the bipod (`PlayAtPoint`: whole for the character's
    // own eyes; from outside heard to the set's farthest).
    if let Some(bipod) = sounds.bipod.as_ref().filter(|_| grounds > 0 || !bipod_marks.is_empty()) {
        let at = rig.as_ref().and_then(|rig| rig.bones.get(bipod.node)).and_then(|&node| places.get(node).ok()).map(|place| place.translation());
        let mut due: Vec<(&ClipSet, String)> = bipod_marks.iter().map(|&open| if open { (&bipod.open, "bipod open".to_owned()) } else { (&bipod.close, "bipod close".to_owned()) }).collect();
        for _ in 0..grounds {
            let surface = match (at, controller.movement.last_mount_data(), range.as_ref()) {
                (Some(at), Some(data), Some(range)) => {
                    use trp_sim::World;
                    let from = [at.x, at.y, -at.z];
                    let way: [f32; 3] = std::array::from_fn(|k| data.point.at[k] - from[k]);
                    let length = (way[0] * way[0] + way[1] * way[1] + way[2] * way[2]).sqrt();
                    if length > 1e-6 {
                        let to: [f32; 3] = std::array::from_fn(|k| from[k] + way[k] / length * BIPOD_SURFACE_RAY);
                        range.surface(from, to).unwrap_or(0)
                    } else {
                        0
                    }
                }
                _ => 0,
            };
            if let Some((_, set)) = bipod.surfaces.iter().find(|(s, _)| *s == surface) {
                due.push((set, format!("bipod on {}", trp_data::audio::sound::SURFACES.get(surface as usize).unwrap_or(&"?"))));
            }
        }
        for (set, what) in due {
            if set.clips.is_empty() {
                continue;
            }
            let drawn = (draws.0.range(0.0, set.clips.len() as f32) as usize).min(set.clips.len() - 1);
            let (name, clip) = &set.clips[drawn];
            let gain = set.volume * overall;
            if let Some(log) = &log {
                use std::io::Write;
                if let Ok(mut out) = log.0.lock() {
                    let _ = writeln!(out, "{:9.3} hands        {:28} {name} volume {gain:.4} {}", time.elapsed_secs(), what, if own_eyes { "whole" } else { "placed" });
                    let _ = out.flush();
                }
            }
            if own_eyes {
                mixer.0.add_to(crate::audio::ui_sound::Group::Guns, Voice::whole(clip, gain));
            } else if let (Ok(camera), Some(at)) = (camera.single(), at) {
                let to_source = at - camera.translation();
                let distance = to_source.length();
                if distance <= set.max_distance {
                    let falloff = if distance > FULL_WITHIN { FULL_WITHIN / distance } else { 1.0 };
                    let side = if distance > 1e-4 { (to_source / distance).dot(camera.right().as_vec3()).clamp(-1.0, 1.0) } else { 0.0 };
                    let angle = (side + 1.0) * std::f32::consts::FRAC_PI_4;
                    let level = gain * falloff;
                    let pitch: Pitch = Arc::new(std::sync::atomic::AtomicU32::new(1.0f32.to_bits()));
                    mixer.0.add_to(crate::audio::ui_sound::Group::Guns, Voice::placed(clip, [angle.cos() * level, angle.sin() * level], pitch));
                }
            }
        }
    }
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
                    None => mixer.0.add_to(crate::audio::ui_sound::Group::Guns, Voice::whole(clip, gain)),
                    // Past the bank's rolloff it is not heard (`PlayClip`).
                    Some((distance, _)) if distance > bank.rolloff => {}
                    Some((_, gains)) => {
                        let pitch: Pitch = Arc::new(std::sync::atomic::AtomicU32::new(1.0f32.to_bits()));
                        mixer.0.add_to(crate::audio::ui_sound::Group::Guns, Voice::placed(clip, [gains[0] * gain, gains[1] * gain], pitch));
                    }
                }
            }
        }
    }
    // The weapon's mount sound (`WeaponSoundPlayer.PlayMountSound`): one of
    // the bank's clips, at its random volume times the aiming sound's
    // (`VolumeMultByWeaponErgonomic`), at the weapon.
    if let (Some((kind_ergonomics, (bank, delta))), true) = (sounds.aiming.zip(aiming.mount.clone()), mounts > 0) {
        let (_, ergonomics) = kind_ergonomics;
        if !bank.clips.is_empty() {
            let drawn = (draws.0.range(0.0, bank.clips.len() as f32) as usize).min(bank.clips.len() - 1);
            let (name, clip) = &bank.clips[drawn];
            let random = if delta > 0.0 { bank.volume + draws.0.range(-delta, 0.0) } else { bank.volume };
            let gain = random * aiming_volume(ergonomics, controller.movement.stamina.settings.overweight) * overall;
            if let Some(log) = &log {
                use std::io::Write;
                if let Ok(mut out) = log.0.lock() {
                    let _ = writeln!(out, "{:9.3} hands        {:28} {name} volume {gain:.4} {}", time.elapsed_secs(), "mount", if own_eyes { "whole" } else { "placed" });
                    let _ = out.flush();
                }
            }
            if own_eyes {
                mixer.0.add_to(crate::audio::ui_sound::Group::Guns, Voice::whole(clip, gain));
            } else if let Ok(camera) = camera.single() {
                let p = controller.movement.position;
                let to_source = Vec3::new(p[0], p[1] + HANDS_HIGH, -p[2]) - camera.translation();
                let distance = to_source.length();
                if distance <= bank.rolloff {
                    let falloff = if distance > FULL_WITHIN { FULL_WITHIN / distance } else { 1.0 };
                    let side = if distance > 1e-4 { (to_source / distance).dot(camera.right().as_vec3()).clamp(-1.0, 1.0) } else { 0.0 };
                    let angle = (side + 1.0) * std::f32::consts::FRAC_PI_4;
                    let level = gain * falloff;
                    let pitch: Pitch = Arc::new(std::sync::atomic::AtomicU32::new(1.0f32.to_bits()));
                    mixer.0.add_to(crate::audio::ui_sound::Group::Guns, Voice::placed(clip, [angle.cos() * level, angle.sin() * level], pitch));
                }
            }
        }
    }
    // The rig's own marks, then its launcher's on the launcher's player.
    let all = marks.into_iter().map(|mark| (mark, false)).chain(launcher_marks.into_iter().map(|mark| (mark, true)));
    for (mark, launcher) in all {
        let player = if launcher { sounds.launcher.as_deref() } else { Some(&**sounds) };
        let Some(element) = player.and_then(|player| player.for_mark(&mark)) else { continue };
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
            mixer.0.add_to(crate::audio::ui_sound::Group::Guns, Voice::whole(clip, gain));
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
        mixer.0.add_to(crate::audio::ui_sound::Group::Guns, Voice::placed(clip, [angle.cos() * level, angle.sin() * level], pitch));
    }
}
