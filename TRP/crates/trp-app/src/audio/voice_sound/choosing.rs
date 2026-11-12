//! Voices read, one chosen and kept, and its preview played.

use super::*;

/// Starts reading the voice away from the frame, if the options' `HEALTH
/// SYSTEM` is on; off, nothing is read and nothing is said. The voice is
/// the one named (the SOUNDS page's PLAYER VOICE), else the profile's.
pub fn start_reading(app: &mut App, install: trp_data::Install, on: bool, name: Option<String>) {
    app.add_message::<PhraseAsked>().add_message::<PhraseTold>().init_resource::<Voiced>();
    if !on {
        return;
    }
    let exhaustion_heard = app.world().resource::<crate::GameData>().db.config.pointer("/Stamina/StaminaExhaustionStartsBreathSound").and_then(serde_json::Value::as_bool).unwrap_or(true);
    let (receive, ask, decoded) = spawn_reading(install.clone(), name);
    app.insert_resource(VoiceSounds {
        speaker: Speaker::new(0x564f_4943),
        clips: Vec::new(),
        crack: None,
        speech: None,
        character: None,
        coming: Some(Mutex::new(receive)),
        playing: None,
        parts: [0.0; 7],
        audible: false,
        shield: None,
        exhaustion_heard,
        logged: 0,
        ask: Some(ask),
        decoded: Some(Mutex::new(decoded)),
        asked: Vec::new(),
        waiting: None,
        shots: 0,
        install,
    });
    app.add_systems(Update, (change_voice, speak).chain().run_if(resource_exists::<VoiceSounds>).after(crate::player::controller::step));
}

/// Reads a voice on a thread of its own (the one named, else the
/// profile's): every bank's tags and lengths, the sounds of the five the
/// health says at once, the crack and the groups' presets, sent as one
/// `Read`; then each bank asked for, its sounds sent as they are read. The
/// thread ends once its asking side is let go of (a voice chosen anew).
fn spawn_reading(install: trp_data::Install, name: Option<String>) -> (Receiver<Read>, Sender<i32>, Receiver<trp_data::audio::sound::PhraseBank>) {
    let (send, receive) = channel();
    let (ask, asked) = channel::<i32>();
    let (send_bank, decoded) = channel();
    std::thread::spawn(move || {
        let Some(name) = name.or_else(|| install.profile_voice()) else {
            eprintln!("the profile names no voice; the character says nothing");
            return;
        };
        // Every bank, the sounds of the five the health says at once.
        let read = trp_data::audio::sound::VoiceReader::open(&install, &name).and_then(|mut reader| {
            let banks = reader.banks(&|_| true, &|trigger| trp_sim::sound::speaker::trigger::READ.contains(&trigger))?;
            Ok((reader, banks))
        });
        let (reader, banks) = match read {
            Ok(read) => read,
            Err(e) => {
                eprintln!("the voice {name} could not be read; the character says nothing: {e:#}");
                return;
            }
        };
        let voice = trp_data::audio::sound::VoiceBanks { name: reader.name.clone(), bundle: reader.bundle.clone(), banks };
        // Its bundles let go of until a bank is wanted.
        drop(reader);
        let crack = trp_data::audio::sound::fracture_sound(&install).map_err(|e| eprintln!("a bone's crack could not be read; it is silent: {e:#}")).ok();
        let presets = trp_data::audio::audio_preset::audio_group_presets(&install).map_err(|e| eprintln!("the audio groups' presets could not be read; the voice at the clips' own volume: {e:#}")).unwrap_or_default();
        let find = |group: i32| presets.iter().find(|p| p.group == group).cloned();
        let _ = send.send(Read { voice, crack, speech: find(SPEECH), character: find(CHARACTER) });
        // Then any other bank's sounds, as each is first said: the voice
        // opened for those asked meanwhile, and let go of again.
        while let Ok(first) = asked.recv() {
            let wanted: Vec<i32> = std::iter::once(first).chain(asked.try_iter()).collect();
            let read = trp_data::audio::sound::VoiceReader::open(&install, &name).and_then(|mut reader| reader.banks(&|t| wanted.contains(&t), &|_| true));
            match read {
                Ok(banks) => banks.into_iter().for_each(|bank| {
                    let _ = send_bank.send(bank);
                }),
                Err(e) => eprintln!("the voice {name}'s phrases {wanted:?} could not be read; they are silent: {e:#}"),
            }
        }
    });
    (receive, ask, decoded)
}

/// A voice chosen on the SOUNDS page (its name, `Bear_1`): the character
/// speaks with it from then on.
#[derive(Message, Clone, Debug)]
pub struct VoiceChosen(pub String);

/// The voice changed at once: what it says cut, its banks let go of and the
/// new voice read as the run's first was (`spawn_reading`); the speaker
/// keeps all else of the character, and is silent until the new banks
/// come (a fraction of a second, on the reading thread). The game changes a
/// profile's voice out of a raid only; this is the app's, for its options.
fn change_voice(mut voice: ResMut<VoiceSounds>, mut chosen: MessageReader<VoiceChosen>, mixer: Option<Res<Mixer>>, mut voiced: ResMut<Voiced>) {
    let Some(VoiceChosen(name)) = chosen.read().last().cloned() else { return };
    let voice = &mut *voice;
    if let (Some(cut), Some(mixer)) = (voice.playing.take(), mixer.as_deref()) {
        cut.store(mixer.0.clock(), Ordering::Release);
    }
    let (coming, ask, decoded) = spawn_reading(voice.install.clone(), Some(name.clone()));
    (voice.coming, voice.ask, voice.decoded) = (Some(Mutex::new(coming)), Some(ask), Some(Mutex::new(decoded)));
    voice.clips.clear();
    voice.asked.clear();
    voice.waiting = None;
    voice.speaker.set_banks(Vec::new());
    voice.speaker.take_said();
    voiced.0.clear();
    info!("the voice chosen: {name}");
}

/// Where the voice chosen is kept in the options' file.
pub const KEPT_AS: &str = "voice";

/// The voice the options' file keeps, by its name.
pub fn kept(file: &std::path::Path) -> Option<String> {
    let all = crate::platform::options_store::read(file)?;
    all.get(KEPT_AS)?.as_str().map(str::to_owned)
}

/// Keeps the voice chosen in the options' file, all else in it as it was;
/// none takes it out (the profile's is then spoken with).
pub fn keep(file: &std::path::Path, name: Option<&str>) {
    crate::platform::options_store::keep(file, KEPT_AS, name.map(|name| serde_json::json!(name)), "the voice chosen");
}

/// The voice spoken with: the one the options' file keeps where the game
/// still offers it, else the profile's own.
pub fn chosen(offered: &[trp_data::setup::install::VoiceChoice], file: Option<&std::path::Path>, profile: Option<String>) -> Option<String> {
    file.and_then(kept).filter(|name| offered.iter().any(|voice| voice.name == *name)).or(profile)
}

/// A voice to be heard as it is on the SOUNDS page (its name): a mutter of
/// it, as the game's voice list plays one (`VoicePreviewSpeaker`).
#[derive(Message, Clone, Debug)]
pub struct PreviewAsked(pub String);

/// The voices heard on the SOUNDS page: each one's mutters once read, the
/// one being read and the one waited for, what plays, and the draw.
#[derive(Resource)]
pub struct VoicePreview {
    pub(super) install: trp_data::Install,
    pub(super) heard: std::collections::HashMap<String, Vec<Samples>>,
    reading: Option<(String, Mutex<Receiver<Vec<Samples>>>)>,
    wanted: Option<String>,
    pub(super) playing: Option<Cut>,
    dice: u64,
}

/// Readies the SOUNDS page's voices to be heard, whatever the switches.
pub fn start_preview(app: &mut App, install: trp_data::Install) {
    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0x9e37_79b9, |t| t.as_nanos() as u64);
    app.add_message::<VoiceChosen>().add_message::<PreviewAsked>().insert_resource(VoicePreview { install, heard: Default::default(), reading: None, wanted: None, playing: None, dice: seed | 1 });
    app.add_systems(Update, preview.after(crate::player::controller::step));
}

/// A voice heard as the game's list plays one (`VoicePreviewSpeaker.Play`):
/// a clip of its mutter's bank (`PlayerVoiceLoader.TakeVoice`, its
/// default `OnMutter`) drawn at even odds (`Random.Range(0, Clips.Length)`),
/// forced on the interface (`GUISounds.ForcePlaySound`): the one before it
/// stopped, at its own volume, the interface's held at -17 dB at the least
/// (`Levels::forced_gain`). The bank is read the first time the voice is
/// asked for, away from the frame, and the clip plays as it comes.
fn preview(mut preview: ResMut<VoicePreview>, mut asked: MessageReader<PreviewAsked>, mixer: Option<Res<Mixer>>, levels: Option<Res<Levels>>) {
    let preview = &mut *preview;
    if let Some(PreviewAsked(name)) = asked.read().last().cloned() {
        if !preview.heard.contains_key(&name) && preview.reading.as_ref().is_none_or(|(reading, _)| *reading != name) {
            let (send, receive) = channel();
            let install = preview.install.clone();
            let read_name = name.clone();
            std::thread::spawn(move || {
                let read = trp_data::audio::sound::VoiceReader::open(&install, &read_name).and_then(|mut reader| reader.banks(&|trigger| trigger == trp_sim::sound::speaker::trigger::MUTTER, &|_| true));
                let clips = match read {
                    Ok(banks) => banks.into_iter().flat_map(|bank| bank.clips).filter_map(|clip| clip.clip.map(Samples::of_owned)).collect(),
                    Err(e) => {
                        eprintln!("the voice {read_name}'s mutters could not be read; it is not heard: {e:#}");
                        Vec::new()
                    }
                };
                let _ = send.send(clips);
            });
            preview.reading = Some((name.clone(), Mutex::new(receive)));
        }
        preview.wanted = Some(name);
    }
    let arrived = preview.reading.as_ref().and_then(|(name, receive)| receive.lock().ok()?.try_recv().ok().map(|clips| (name.clone(), clips)));
    if let Some((name, clips)) = arrived {
        info!("the voice {name}: {} mutters to hear", clips.len());
        preview.heard.insert(name, clips);
        preview.reading = None;
    }
    let Some(count) = preview.wanted.as_ref().and_then(|name| preview.heard.get(name)).map(Vec::len) else { return };
    let Some(name) = preview.wanted.take() else { return };
    let Some(mixer) = mixer.filter(|_| count > 0) else { return };
    preview.dice = preview.dice.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
    let at = (preview.dice >> 33) as usize % count;
    let clip = preview.heard[&name][at].clone();
    info!("the voice {name} heard: its mutter {at} of {count}");
    if let Some(cut) = preview.playing.take() {
        cut.store(mixer.0.clock(), Ordering::Release);
    }
    let gain = levels.as_deref().map_or(1.0, Levels::forced_gain);
    let cut = no_cut();
    mixer.0.add(Voice::whole(&clip, gain).at_frame(mixer.0.clock(), false, cut.clone()));
    preview.playing = Some(cut);
}
