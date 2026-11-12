//! Sounds read and their clips taken, the mixer started, and sounds played.

use super::*;

/// The clips once read, ready to mix; the source they are mixed into; and
/// what brings the clips.
#[derive(Resource, Default)]
pub struct Sounds {
    /// The interface's, each with its clip's name.
    pub(super) ui: HashMap<i32, (String, Samples)>,
    /// The things', by their clips' names.
    items: HashMap<String, Samples>,
    /// The magazine's lists, in the game's order, with their names.
    pub(super) load: Vec<(String, Samples)>,
    unload: Vec<(String, Samples)>,
    notification: Option<Samples>,
    /// The dice for which of a list plays.
    dice: u64,
    pub(super) mixed: Mixed,
    coming: Option<Mutex<Receiver<(Read, Samples)>>>,
}

/// The one source the app's sounds are mixed into, for the other kinds of
/// sound (`step_sound`).
#[derive(Resource, Clone)]
pub struct Mixer(pub Mixed);

/// Starts reading the clips from the install away from the frame.
pub fn start_reading(app: &mut App, install: trp_data::Install) {
    let (send, receive) = channel();
    let sounds = Sounds { coming: Some(Mutex::new(receive)), ..default() };
    let game = sounds.mixed.game.clone();
    std::thread::spawn(move || {
        // The game's own mixer first: what the mix does with all of it.
        match trp_data::audio::sound::master_mixer(&install) {
            Ok(mixer) => {
                let gunshots = mixer.group("Gunshots").map_or(0.0, |group| mixer.path_db(group));
                let movement = mixer.group("ClientPlayerMovement").map_or(0.0, |group| mixer.path_db(group));
                let speech = mixer.group("ClientPlayerSpeech").map_or(0.0, |group| mixer.path_db(group));
                let self_speech = mixer.group("ClientPlayerSelfSpeechReverb").map_or(0.0, |group| mixer.path_db(group));
                let observed_speech = mixer.group("ObservedPlayerSpeech").map_or(0.0, |group| mixer.path_db(group));
                let occluded = mixer.group("SimpleOccluded").map_or(0.0, |group| mixer.path_db(group));
                let gain = |db: f32| 10f32.powf(db / 20.0);
                // What the headphones' templates set, as the raid begins.
                let start: Vec<f32> = trp_sim::sound::hearing::Param::ALL.iter().filter_map(|param| mixer.value(param.name())).collect();
                let headphones_start = start.try_into().ok();
                if headphones_start.is_none() {
                    warn!("the game's mixer exposes not all a headphones template sets; a template's first fade begins at the default's");
                }
                let read = GameMix {
                    compressor: mixer.master_compressor(),
                    gunshots: gain(gunshots),
                    movement: gain(movement),
                    speech: gain(speech),
                    self_speech: gain(self_speech),
                    observed_speech: gain(observed_speech),
                    occluded: gain(occluded),
                    headphones_start,
                };
                info!(
                    "the game's mixer ({} groups, from its {} snapshot): the master's effect {:?} (not applied), the gunshots' group {gunshots:+.1} dB, the player's movement's {movement:+.1} dB",
                    mixer.groups.len(),
                    mixer.start_snapshot,
                    read.compressor
                );
                let _ = game.set(read);
            }
            Err(e) => {
                warn!("the game's mixer could not be read; its groups add nothing: {e:#}");
                let _ = game.set(GameMix::unread());
            }
        }
        let kinds: Vec<i32> = Ui::ALL.iter().map(|ui| *ui as i32).collect();
        match trp_data::audio::sound::ui_sounds(&install, &kinds) {
            Ok(clips) => {
                for (kind, clip) in clips.by_kind {
                    let _ = send.send((Read::Ui(kind, clip.name.clone()), Samples::of_owned(clip)));
                }
                for clip in clips.load {
                    let _ = send.send((Read::Load(clip.name.clone()), Samples::of_owned(clip)));
                }
                for clip in clips.unload {
                    let _ = send.send((Read::Unload(clip.name.clone()), Samples::of_owned(clip)));
                }
                if let Some(clip) = clips.notification {
                    let _ = send.send((Read::Notification, Samples::of_owned(clip)));
                }
            }
            Err(e) => eprintln!("the interface's sounds could not be read; it is silent: {e:#}"),
        }
        match trp_data::audio::sound::item_sounds(&install) {
            Ok(clips) => {
                for (name, clip) in clips {
                    let _ = send.send((Read::Item(name), Samples::of_owned(clip)));
                }
            }
            Err(e) => eprintln!("the things' sounds could not be read; they are silent: {e:#}"),
        }
    });
    app.insert_resource(Mixer(sounds.mixed.clone()));
    app.insert_resource(sounds);
}

/// The one source, playing from the start for the whole run.
pub(super) fn start_mixing(mut commands: Commands, sounds: Res<Sounds>, mut sources: ResMut<Assets<Mixed>>) {
    commands.spawn(AudioPlayer(sources.add(sounds.mixed.clone())));
}

pub(super) fn take_clips(mut sounds: ResMut<Sounds>) {
    let Some(coming) = sounds.coming.take() else { return };
    let mut done = false;
    if let Ok(receive) = coming.lock() {
        loop {
            match receive.try_recv() {
                Ok((read, frames)) => match read {
                    Read::Ui(kind, name) => {
                        sounds.ui.insert(kind, (name, frames));
                    }
                    Read::Item(name) => {
                        sounds.items.insert(name, frames);
                    }
                    Read::Load(name) => sounds.load.push((name, frames)),
                    Read::Unload(name) => sounds.unload.push((name, frames)),
                    Read::Notification => sounds.notification = Some(frames),
                },
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    done = true;
                    break;
                }
            }
        }
    }
    if !done {
        sounds.coming = Some(coming);
    } else {
        info!("sounds: {} of the interface's and {} of the things' read", sounds.ui.len(), sounds.items.len());
    }
}

/// The game's volume of a thing's sound in a raid (`GetCommonSoundsVolume`).
const IN_RAID: f32 = 0.15;

/// Plays the sounds asked for, each once, at its volume: the overall
/// volume's and the interface's for the interface's; the overall's and the
/// raid's 0.15 for a thing's (its mixer group's own attenuation not read,
/// `PROVISIONAL`). A thing's group with no clip for the way is silent, as
/// the game's (`GetClip` finds none).
///
/// A round's sound is one of its list, as a thing's is played
/// (`PlaySound` as a common sound), chosen as the game's
/// `UIClipSounds.GetRandomClip` chooses: `Random.Range(0, count - 1)`, which
/// never takes the last.
///
/// With the interface's dump, what is played is told as the game's
/// recorder hears it (`Heard`): the sound asked for (`PlayUISound`,
/// `PlayItemSound`), then the clip, if one is found (`PlaySound`); a round's
/// clip alone, the game's lists calling `PlaySound` straight; nothing of a
/// notice or of the world's sounds, which do not go through `PlaySound`.
pub fn play(mut asked: MessageReader<Sound>, mut sounds: ResMut<Sounds>, levels: Option<Res<Levels>>, mut heard: Option<ResMut<Heard>>) {
    let levels = levels.map_or(Levels { overall: 10, interface: 10 }, |levels| *levels);
    let interface = Levels::gain(levels.overall) * Levels::gain(levels.interface);
    let sounds = &mut *sounds;
    for sound in asked.read() {
        let mut pick = |list: &Vec<(String, Samples)>| {
            sounds.dice = sounds.dice.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            let upto = list.len().saturating_sub(1).max(1);
            list.get((sounds.dice >> 33) as usize % upto).cloned()
        };
        let item = |name: String| sounds.items.get(&name).map(|clip| (name, clip.clone()));
        let open = |group: &str| item(format!("{group}_open"));
        // The clip and its name, its gain, and what is told of it: the
        // sound asked for, and whether the clip is a common sound.
        let (clip, gain, asked_for, common) = match sound {
            Sound::Ui(ui) => (sounds.ui.get(&(*ui as i32)).cloned(), interface, Some(Told::Ui(ui.game_name())), false),
            // Its own clip played straight, or the interface's sound
            // (`ItemUiContext.OpenItem`).
            Sound::Item(group, Way::Open) => match open(group) {
                Some(own) => (Some(own), interface, None, false),
                None => (sounds.ui.get(&(Ui::OpenContainer as i32)).cloned(), interface, Some(Told::Ui(Ui::OpenContainer.game_name())), false),
            },
            Sound::Item(group, way) => (item(format!("{group}_{}", way.name())), Levels::gain(levels.overall) * IN_RAID, Some(Told::Item(group.clone(), way.name())), true),
            Sound::Load => (pick(&sounds.load.clone()), Levels::gain(levels.overall) * IN_RAID, None, true),
            Sound::Unload => (pick(&sounds.unload.clone()), Levels::gain(levels.overall) * IN_RAID, None, true),
            Sound::Notification => (sounds.notification.clone().map(|clip| (String::new(), clip)), interface, None, false),
            Sound::Taken(group) => {
                if let Some(clip) = sounds.items.get(&format!("{group}_{}", Way::Pickup.name())) {
                    sounds.mixed.add_to(Group::CommonSounds, Voice::whole(clip, Levels::gain(levels.overall)));
                }
                continue;
            }
            Sound::Looping(name, own, cut, level) => {
                if let Some(clip) = sounds.items.get(name) {
                    let starts = sounds.mixed.clock();
                    sounds.mixed.add_to(Group::CommonSounds, Voice::whole(clip, Levels::gain(levels.overall) * own).at_frame(starts, true, cut.clone()).following_level(level.clone()));
                }
                continue;
            }
        };
        if let Some(heard) = heard.as_deref_mut() {
            heard.0.extend(asked_for);
            if let Some((name, _)) = clip.as_ref().filter(|_| !matches!(sound, Sound::Notification)) {
                heard.0.push(Told::Clip(name.clone(), common));
            }
        }
        if let Some((_, clip)) = &clip {
            sounds.mixed.add(Voice::whole(clip, gain));
        }
    }
}
