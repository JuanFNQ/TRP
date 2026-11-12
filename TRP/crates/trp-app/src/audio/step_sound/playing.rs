//! Steps played for the character and the dummies, and bodies' falls.

use super::*;

/// A line for every sound of moving played, to a file (`--log-sounds`):
/// the run's time, the surface, the bank, the clips with their shares,
/// the pitch, the volume asked and the two channels' gains.
#[derive(Resource)]
pub struct SoundLog(pub Mutex<std::io::BufWriter<std::fs::File>>);

/// Plays what the rules asked for on the frame's steps.
pub(super) fn play(
    mut controller: ResMut<Controller>,
    mut banks: ResMut<StepBanks>,
    mixer: Option<Res<Mixer>>,
    levels: Option<Res<Levels>>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    log: Option<Res<SoundLog>>,
    time: Res<Time>,
    carried: Option<Res<crate::screens::inventory_view::Carried>>,
) {
    // What is worn on the head as it stands after the frame's keys and
    // moves: a face shield, night vision or thermal vision come down or
    // up, put on or taken off with it on (`trp_sim::visor::SwitchSounds`).
    if let Some(carried) = carried.as_deref() {
        let heard = controller.switch_sounds.hear(&carried.equipment);
        for &sound in &heard {
            info!("a switch on the head heard: {}", trp_data::audio::sound::TOGGLE_SOUNDS[sound]);
        }
        controller.toggle_sounds.extend(heard);
    }
    let sounds = &mut controller.movement.sounds;
    if !banks.figures.is_empty() && !sounds.has_figures() {
        sounds.set_figures(banks.figures.clone());
        // And the kit's banks' (a mount's medium rattle draws its volume).
        if let Some(read) = banks.sounds.as_ref() {
            sounds.set_kit_figures(std::array::from_fn(|which| read.gear.get(which).copied().flatten().map(|at| Figures::of(&read.banks[at]))));
        }
    }
    let asked = sounds.take();
    // A switch on the head's sound: the character's own, not placed in the
    // world, at full volume through the movement's group
    // (`ClientPlayer.PlayToggleSound`).
    let toggled = std::mem::take(&mut controller.toggle_sounds);
    if let Some(mixer) = mixer.as_deref() {
        let gain = levels.as_deref().map_or(1.0, |levels| levels.overall_gain()) * mixer.0.movement_gain();
        for clip in toggled.into_iter().filter_map(|at| banks.toggles.get(at)?.as_ref()) {
            mixer.0.add_to(crate::audio::ui_sound::Group::ClientPlayer, Voice::whole(clip, gain));
        }
    }
    let (Some(mixer), Ok(camera)) = (mixer, camera.single()) else { return };
    if asked.is_empty() || banks.sounds.is_none() {
        return;
    }
    let overall = levels.map_or(1.0, |levels| levels.overall_gain()) * mixer.0.movement_gain();
    // From the point over the feet to the camera: the falloff and the pan.
    let p = controller.movement.position;
    let source = Vec3::new(p[0], p[1] + OVER_FEET, -p[2]);
    let to_source = source - camera.translation();
    let distance = to_source.length();
    let outdoors = controller.movement.sounds.settings.outdoor_rolloff;
    let side = if distance > 1e-4 { (to_source / distance).dot(camera.right().as_vec3()).clamp(-1.0, 1.0) } else { 0.0 };
    let angle = (side + 1.0) * std::f32::consts::FRAC_PI_4;
    let pan = [angle.cos(), angle.sin()];
    let banks = &mut *banks;
    let Some(sounds) = banks.sounds.as_ref() else { return };
    for play in asked {
        // A vault's play names its element or its kit bank (footsteps
        // spec, "The vault's sounds"): the element's bank at the height's
        // place in its volume range, the kit's at the height's share of
        // its own; either times the bank's base volume and its drawn
        // volume, as a bank played at a point is (`PlayAtPoint`), from a
        // source of its own with its own drawn pitch.
        let vault = matches!(play.bank, BankOf::Vault { .. } | BankOf::VaultGear(_));
        let (at, volume) = match play.bank {
            BankOf::Surface(surface, way) => (sounds.surface(surface).and_then(|set| set.banks.get(way).copied().flatten()), play.volume),
            BankOf::Kit(which) => (sounds.gear.get(which).copied().flatten(), play.volume),
            BankOf::Vault { kind, mark, surface } => match sounds.vault_element(surface, kind, mark) {
                Some(element) => (element.bank, element.min_volume + (element.max_volume - element.min_volume) * play.volume.clamp(0.0, 1.0)),
                None => (None, 0.0),
            },
            BankOf::VaultGear(kind) => (sounds.vault_gear.get(kind).copied().flatten(), play.volume),
        };
        let Some(at) = at else { continue };
        // The source's farthest: the kind's reach, or a vault's bank's own.
        let farthest = match play.reach {
            Some(reach) => FIRST_PERSON_ROLLOFF * reach * outdoors,
            None => sounds.banks[at].rolloff,
        };
        let falloff = match banks.character.as_ref() {
            Some(preset) => preset.rolloff_gain(distance, farthest).max(0.0) * preset.overall_volume,
            None if distance > FULL_WITHIN => FULL_WITHIN / distance,
            None => 1.0,
        };
        let volume = if vault {
            let bank = &sounds.banks[at];
            let drawn = if bank.delta_volume > 0.0 { bank.base_volume + banks.rng.range(-bank.delta_volume, 0.0) } else { bank.base_volume };
            volume * bank.base_volume * drawn
        } else {
            volume
        };
        let choice = choose(&sounds.banks[at], &mut banks.shuffles[at], play.blend, &mut banks.rng);
        let pitch: Pitch = if vault {
            Arc::new(AtomicU32::new(choice.pitch.to_bits()))
        } else {
            let cell = if play.kit { &banks.kit } else { &banks.steps };
            cell.store(choice.pitch.to_bits(), Ordering::Relaxed);
            cell.clone()
        };
        let volume = volume.clamp(0.0, 1.0) * if play.kit { KIT_VOLUME } else { 1.0 } * overall * falloff;
        let both = choice.clips.iter().all(Option::is_some);
        let mut said = Vec::new();
        for (n, clip) in choice.clips.iter().enumerate() {
            let Some((place, samples)) = clip.and_then(|clip| Some((clip, banks.clips.get(clip)?.as_ref()?))) else { continue };
            let share = if !both {
                1.0
            } else if n == 0 {
                choice.share
            } else {
                1.0 - choice.share
            };
            let gain = volume * share;
            mixer.0.add_to(crate::audio::ui_sound::Group::ClientPlayer, Voice::placed(samples, [pan[0] * gain, pan[1] * gain], pitch.clone()));
            said.push(format!("{} x{share:.3}", banks.names[place]));
        }
        if let Some(log) = &log {
            use std::io::Write;
            let surface = match play.bank {
                BankOf::Surface(surface, _) | BankOf::Vault { surface, .. } => trp_data::audio::sound::SURFACES.get(surface as usize).copied().unwrap_or("?"),
                BankOf::Kit(_) | BankOf::VaultGear(_) => "kit",
            };
            if let Ok(mut out) = log.0.lock() {
                let _ = writeln!(
                    out,
                    "{:9.3} {surface:12} {:28} blend {:.3} asked {:.4} volume {:.4} pitch {:.3} gains {:.4} {:.4} distance {distance:.3} farthest {farthest:.2} falloff {falloff:.4} [{}]",
                    time.elapsed_secs(),
                    sounds.banks[at].name,
                    play.blend,
                    play.volume,
                    volume,
                    choice.pitch,
                    pan[0] * volume,
                    pan[1] * volume,
                    said.join(", ")
                );
                let _ = out.flush();
            }
        }
    }
}

/// The dummies' sounds of moving, heard from outside (footsteps spec, "A
/// character seen from outside"): each living dummy's rules run as the
/// player's do, seen from outside (`StepSounds::third_person`), and what
/// they ask for is taken every frame and played as the game plays another
/// character's, from its two sources (its steps', its kit's) 0.1 m over its
/// feet:
///
/// - each sound's source heard no farther than the movement's base rolloff
///   for a character seen from outside (`BaseMaxMovementRolloff`, 90 m) times
///   the kind's multiplier its rolloff was set by, times the outdoors' share
///   (`OutdoorRolloffMult`, `PROVISIONAL`: the app's range has no indoors);
///   past it, nothing (`SoundBank.Play`);
/// - every sound but a footstep chooses its bank's list by the distance;
/// - its volume clamped to 0 to 1, the kit's source at 1
///   (`LOCAL_AI_GEAR_VOLUME`), falling by the `Character` group's rolloff
///   curve over the source's farthest (its `SoundRolloff`, from the
///   install's preset) at its `OverallVolume`; through the mixer's
///   `ObservedPlayerMovement` group, at 0 dB with the groups above it (not
///   the player's own -3.5 dB); panned at equal power (`PROVISIONAL`: the
///   game's binaural plug-in and its occlusion are not built);
/// - the pitch of each source its own, a dummy's own two.
///
/// With `MOVEMENT SOUNDS` off nothing plays; the asks are taken all the
/// same.
#[allow(clippy::too_many_arguments)]
pub(super) fn play_dummies(
    dummies: Option<ResMut<crate::world::dummy::Dummies>>,
    mut banks: ResMut<StepBanks>,
    mixer: Option<Res<Mixer>>,
    levels: Option<Res<Levels>>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    log: Option<Res<SoundLog>>,
    time: Res<Time>,
) {
    let Some(mut dummies) = dummies else { return };
    let banks = &mut *banks;
    let overall = levels.map_or(1.0, |levels| levels.overall_gain());
    let camera = camera.single().ok();
    for (which, feet, rules) in dummies.movers() {
        if !banks.figures.is_empty() && !rules.has_figures() {
            rules.set_figures(banks.figures.clone());
            // How far from the listener its steps are looked up: the
            // longest of every surface's bank of each kind.
            if let Some(sounds) = banks.sounds.as_ref() {
                rules.settings.widen_surface_checks(sounds.surfaces.iter().flat_map(|set| set.banks.iter().enumerate().filter_map(|(way, at)| at.map(|at| (way, sounds.banks[at].rolloff)))));
            }
        }
        let asked = rules.take();
        let (Some(mixer), Some(camera), Some(sounds)) = (mixer.as_deref(), camera, banks.sounds.as_ref()) else { continue };
        if asked.is_empty() {
            continue;
        }
        while banks.dummy_pitches.len() <= which {
            banks.dummy_pitches.push(std::array::from_fn(|_| Arc::new(AtomicU32::new(1.0f32.to_bits()))));
        }
        let source = Vec3::new(feet[0], feet[1] + OVER_FEET, -feet[2]);
        let to_source = source - camera.translation();
        let distance = to_source.length();
        let side = if distance > 1e-4 { (to_source / distance).dot(camera.right().as_vec3()).clamp(-1.0, 1.0) } else { 0.0 };
        let angle = (side + 1.0) * std::f32::consts::FRAC_PI_4;
        let settings = &rules.settings;
        for play in asked {
            // A vault's or a climb's: the dummies make neither.
            let at = match play.bank {
                BankOf::Surface(surface, way) => sounds.surface(surface).and_then(|set| set.banks.get(way).copied().flatten()),
                BankOf::Kit(which) => sounds.gear.get(which).copied().flatten(),
                BankOf::Vault { .. } | BankOf::VaultGear(_) => None,
            };
            let Some(at) = at else { continue };
            let farthest = settings.third_person_rolloff * play.reach.unwrap_or(1.0) * settings.outdoor_rolloff;
            if distance > farthest {
                continue;
            }
            let blend = if play.by_distance { distance } else { play.blend };
            let choice = choose(&sounds.banks[at], &mut banks.shuffles[at], blend, &mut banks.rng);
            let pitch = banks.dummy_pitches[which][usize::from(play.kit)].clone();
            pitch.store(choice.pitch.to_bits(), Ordering::Relaxed);
            let falloff = match banks.character.as_ref() {
                Some(preset) => preset.rolloff_gain(distance, farthest).max(0.0) * preset.overall_volume,
                None if distance > FULL_WITHIN => FULL_WITHIN / distance,
                None => 1.0,
            };
            let volume = play.volume.clamp(0.0, 1.0) * overall * falloff;
            let both = choice.clips.iter().all(Option::is_some);
            for (n, clip) in choice.clips.iter().enumerate() {
                let Some(samples) = clip.and_then(|clip| banks.clips.get(clip)?.as_ref()) else { continue };
                let share = if !both {
                    1.0
                } else if n == 0 {
                    choice.share
                } else {
                    1.0 - choice.share
                };
                let gain = volume * share;
                // A bot's steps: `ObservedPlayerMovement` (`Player.InitAudioController`).
                mixer.0.add_to(crate::audio::ui_sound::Group::ObservedPlayer, Voice::placed(samples, [angle.cos() * gain, angle.sin() * gain], pitch.clone()));
            }
            if let Some(Ok(mut out)) = log.as_ref().map(|log| log.0.lock()) {
                use std::io::Write;
                let _ = writeln!(
                    out,
                    "{:9.3} dummy {which} {:28} blend {blend:.3} asked {:.4} farthest {farthest:.1} falloff {falloff:.4} volume {volume:.4} pitch {:.3} distance {distance:.3}",
                    time.elapsed_secs(),
                    sounds.banks[at].name,
                    play.volume,
                    choice.pitch
                );
                let _ = out.flush();
            }
        }
    }
}

/// A corpse made this frame (`death::die`, the player's; `dummy::
/// fall_dummies`, a dummy's): where its root is (the character's feet as it
/// died, the game's axes), what the line down from there met (`trp_sim::
/// landing::body_fall_line`: a surface, or nothing within its reach), and
/// whether it is the player's own.
#[derive(Message, Clone, Copy, Debug)]
pub struct BodyFell {
    pub at: [f32; 3],
    pub met: Option<u8>,
    pub own: bool,
}

impl BodyFell {
    /// A corpse at `root`, its surface looked for in the world along the
    /// rule's line.
    pub fn at(root: [f32; 3], world: &impl trp_sim::World, own: bool) -> Self {
        let (from, to) = trp_sim::sound::landing::body_fall_line(root);
        Self { at: root, met: world.surface(from, to), own }
    }
}

/// A corpse's fall heard (sound spec, "A body's fall"; `trp_sim::landing`'s
/// rule), at any death, the player's and a dummy's alike: the surface's
/// prone drop, concrete's where the surface has no set, nothing where the
/// line down met nothing, played once where the root is as a thing let fall
/// (`impact_sound::play_at_point`, `BetterAudio.PlayAtPoint`): asked at the
/// drop sounds' curve at 50 (0.59) times the bank's own draw, its clip by
/// the distance and its pitch drawn, unheard past its rolloff (66.5 m), and
/// falling by the curve of its bank's source group (`Character`, the preset
/// the dummies' steps go by) over that rolloff, in the world's
/// `CommonSounds` at the overall volume (the game's occlusion not built).
/// With `MOVEMENT SOUNDS` off nothing is read and nothing plays.
pub(super) fn play_falls(
    mut fell: MessageReader<BodyFell>,
    mut banks: ResMut<StepBanks>,
    mixer: Option<Res<Mixer>>,
    levels: Option<Res<Levels>>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    log: Option<Res<SoundLog>>,
    time: Res<Time>,
) {
    let banks = &mut *banks;
    let (Some(mixer), Ok(camera), Some(sounds), Some(asked)) = (mixer, camera.single(), banks.sounds.as_ref(), banks.fall_volume) else {
        fell.clear();
        return;
    };
    let overall = levels.map_or(1.0, |levels| levels.overall_gain());
    for fall in fell.read() {
        let whose = if fall.own { "the character's own" } else { "a dummy's" };
        let surface = fall.met.and_then(|surface| trp_data::audio::sound::SURFACES.get(surface as usize).copied()).unwrap_or("nothing");
        let Some(at) = trp_sim::sound::landing::body_fall_bank(sounds, fall.met) else {
            info!("{whose} corpse falls silent: the line down met {surface}");
            continue;
        };
        let bank = &sounds.banks[at];
        let played = crate::audio::impact_sound::play_at_point(&mixer, bank, &banks.clips, &mut banks.shuffles[at], &mut banks.rng, crate::platform::convert::position(fall.at), camera, asked, overall, banks.character.as_ref());
        let said = match played {
            Some((distance, drawn)) => format!("asked {asked:.2}, drawn {:.4}, {distance:.2} m off", asked * drawn),
            None => "out of hearing".to_owned(),
        };
        info!("{whose} corpse falls on {surface}: {} {said}", bank.name);
        if let Some(Ok(mut out)) = log.as_ref().map(|log| log.0.lock()) {
            use std::io::Write;
            let _ = writeln!(out, "{:9.3} fall {surface:12} {:28} {whose}: {said}", time.elapsed_secs(), bank.name);
            let _ = out.flush();
        }
    }
}
