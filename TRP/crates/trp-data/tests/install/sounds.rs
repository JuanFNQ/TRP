//! Tests that read clips, shots, blasts, cases, voices and the mixer.

use super::*;

/// The things' sounds kept as Vorbis (`sound.md`, "Vorbis"): each names a
/// setup header the game's engine holds, and decodes to its own count of
/// frames. And an AI-2's hands have their own sound player, whose sounds
/// its animator's marks name (`hands_sounds`).
#[test]
fn vorbis_clips_decode_and_an_item_s_hands_have_their_sounds() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let clips = trp_data::audio::sound::item_vorbis(&install).expect("the things' Vorbis clips");
    assert!(clips.len() >= 10, "{} Vorbis clips", clips.len());
    let mut setups = trp_data::audio::sound::VorbisSetups::new(&install);
    for (name, sample) in &clips {
        let setup = setups.get(sample.setup_crc).unwrap_or_else(|e| panic!("{name}: {e:#}")).to_vec();
        let samples = trp_data::audio::sound::decode_vorbis(sample, &setup).unwrap_or_else(|e| panic!("{name}: {e:#}"));
        assert_eq!(samples.len() as u64, sample.frames * sample.channels as u64, "{name}: frames decoded");
        assert!(samples.iter().any(|s| s.unsigned_abs() > 1000), "{name}: silent");
    }
    let db = Database::load(&install).expect("database");
    let mut assets = Assets::new(install).expect("manifest");
    let path = db.items["5755356824597772cb798962"].props["UsePrefab"]["path"].as_str().expect("the AI-2's hands").to_owned();
    let rig = trp_data::weapons::weapon::assemble_prefab(&mut assets, &path).expect("the AI-2's rig");
    let sounds = trp_data::audio::sound::hands_sounds(&assets, &rig, &mut setups, true).expect("read").expect("a sound player");
    assert_eq!(sounds.player, "BaseSoundPlayer");
    assert!(sounds.shots.is_none(), "an item's hands have no shots");
    for mark in ["Draw", "Open", "Take", "CapRemove", "Inject", "Discard", "Holster"] {
        let element = sounds.for_mark(mark).unwrap_or_else(|| panic!("no sound for the mark {mark}"));
        assert!(!element.clips.is_empty() && element.clips.iter().all(|clip| !clip.samples.is_empty()), "{mark}");
    }
    println!("{} Vorbis clips decoded; the AI-2's {}: {} sounds", clips.len(), sounds.player, sounds.additional.len());
}

/// A weapon's banks of shots (`WeaponSoundPlayer`): the AKM's fire a loop
/// of sixteen beats at its rate of 600 a minute and a tail, with silenced
/// banks; the SV-98's each shot whole; the MP-43's both barrels' own bank.
#[test]
fn a_weapon_s_shots_are_read() {
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let db = Database::load(&install).expect("database");
    let mut setups = trp_data::audio::sound::VorbisSetups::new(&install);
    let mut assets = Assets::new(install).expect("manifest");
    let mut shots_of = |template: &str| {
        let weapon = trp_data::weapons::weapon::assemble(&mut assets, &db, template).expect("the weapon");
        let sounds = trp_data::audio::sound::hands_sounds(&assets, &weapon, &mut setups, true).expect("read").expect("a sound player");
        assert_eq!(sounds.player, "WeaponSoundPlayer", "{template}");
        sounds.shots.expect("its shots")
    };
    let close = |shots: &trp_data::audio::sound::ShotSounds, bank: &str| -> f32 {
        let bank = shots.bank(bank).unwrap_or_else(|| panic!("no {bank}"));
        let clip = shots.clips[bank.lists[0][0]].1.as_ref().expect("the close clip read");
        clip.samples.len() as f32 / clip.channels as f32 / clip.frequency as f32
    };
    let akm = shots_of("59d6088586f774275f37482f");
    assert!(!akm.non_auto);
    let body = akm.bank("Body").expect("the AKM's body");
    // Sixteen beats of 60 / 600 s.
    assert!((body.clip_length - 1.6).abs() < 1e-4, "{}", body.clip_length);
    assert!((close(&akm, "Body") - 1.6).abs() < 1e-3);
    assert!(close(&akm, "Tail") > 3.0);
    assert!(akm.bank("BodySilenced").is_some() && akm.bank("Doublet").is_none());
    assert_eq!(body.blend.as_deref(), Some(&[5.0, 30.0, 55.0, 100.0][..]));
    let sv98 = shots_of("55801eed4bdc2d89578b4588");
    assert!(sv98.non_auto && sv98.bank("Tail").is_none());
    let mp43 = shots_of("5580223e4bdc2d1c128b457f");
    assert!(mp43.non_auto);
    assert_eq!(mp43.bank("Doublet").expect("both barrels").lists[0].len(), 2);
    let silenced = trp_data::weapons::weapon::assemble(&mut assets, &db, "59d6088586f774275f37482f").expect("the AKM").silenced;
    assert!(!silenced, "the stock AKM has no silencer");
}

/// A contusion's ringing (`Sounds.TinnitusSound`) is read whole.
#[test]
fn the_tinnitus_sound_is_read() {
    let Ok(install) = Install::locate() else { return };
    let clip = trp_data::audio::sound::tinnitus_sound(&install).expect("the tinnitus");
    println!("{}: {} channels at {} Hz, {:.2} s", clip.name, clip.channels, clip.frequency, clip.samples.len() as f32 / clip.channels as f32 / clip.frequency as f32);
    assert!(clip.samples.len() > clip.frequency as usize / 2);
}

/// Every grenade's blast's sound (`ExplosionEffectType`'s effect's bank):
/// the F-1's `Grenade_new` has one, with clips.
#[test]
fn the_grenades_blasts_sounds_are_read() {
    let Ok(install) = Install::locate() else { return };
    let db = trp_data::Database::load(&install).expect("database");
    let mut names: Vec<String> = db.items.values().filter_map(|item| item.props.get("ExplosionEffectType")?.as_str().map(str::to_owned)).filter(|name| !name.is_empty()).collect();
    names.sort();
    names.dedup();
    let sounds = trp_data::audio::sound::effect_sounds(&install, &names, false).expect("the effects' sounds");
    for (name, at) in &sounds.effects {
        match at.map(|at| &sounds.banks[at]) {
            Some(bank) => println!(
                "{name}: {} volume {} -{} pitch {} ±{} rolloff {} source {} physical {} limited {} blend {:?} clips {:?}",
                bank.name,
                bank.base_volume,
                bank.delta_volume,
                bank.base_pitch,
                bank.delta_pitch,
                bank.rolloff,
                bank.source_type,
                bank.physical,
                bank.limited,
                bank.blend,
                bank.lists.iter().map(|list| list.iter().map(|&clip| sounds.clips[clip].0.as_str()).collect::<Vec<_>>()).collect::<Vec<_>>()
            ),
            None => println!("{name}: no sound"),
        }
    }
    let f1 = sounds.bank("Grenade_new").expect("the F-1's blast's bank");
    assert!(f1.lists.iter().any(|list| !list.is_empty()));
}

/// The spent cases' sounds: a bank for every calibre's class and surface,
/// each with clips.
#[test]
fn the_spent_cases_sounds_are_read() {
    let Ok(install) = Install::locate() else { return };
    let sounds = trp_data::audio::sound::shell_sounds(&install, false).expect("the spent cases' sounds");
    for (c, caliber) in trp_data::audio::sound::SHELL_CALIBERS.iter().enumerate() {
        for (s, surface) in trp_data::audio::sound::SHELL_SURFACES.iter().enumerate() {
            let bank = &sounds.banks[sounds.bank(c as u8, s).unwrap_or_else(|| panic!("no bank for {caliber} on {surface}"))];
            println!(
                "{caliber} {surface}: {} volume {} -{} pitch {} ±{} rolloff {} source {} blend {:?} clips {:?}",
                bank.name,
                bank.base_volume,
                bank.delta_volume,
                bank.base_pitch,
                bank.delta_pitch,
                bank.rolloff,
                bank.source_type,
                bank.blend,
                bank.lists.iter().map(Vec::len).collect::<Vec<_>>()
            );
            assert!(bank.lists.iter().any(|list| !list.is_empty()));
        }
    }
}

/// Every PMC voice the profile has (the options' PLAYER VOICE: both sides,
/// the game's order) is found in the install's phrase bundles and has
/// mutters to be heard (`VoicePreviewSpeaker`: its `OnMutter` bank).
#[test]
fn every_pmc_voice_offered_has_mutters() {
    let Ok(install) = Install::locate() else { return };
    let voices = install.profile_voices();
    if voices.is_empty() {
        eprintln!("skipped: the install has no profile");
        return;
    }
    let names: Vec<&str> = voices.iter().map(|voice| voice.name.as_str()).collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted, "ordered by name");
    assert!(voices.iter().any(|voice| voice.usec) && voices.iter().any(|voice| !voice.usec), "both sides: {names:?}");
    for voice in &voices {
        let mut reader = trp_data::audio::sound::VoiceReader::open(&install, &voice.name).expect("the voice's bundle");
        let banks = reader.banks(&|trigger| trigger == 28, &|_| false).expect("its banks");
        let mutters = banks.iter().map(|bank| bank.clips.len()).sum::<usize>();
        println!("{} ({}): {mutters} mutters", voice.name, if voice.usec { "USEC" } else { "BEAR" });
        assert!(mutters > 0, "{} has no mutter", voice.name);
    }
}

/// The mixer's sends are read with their levels, and every name the
/// headphones' templates set is exposed (sound spec, "Hearing under
/// headgear"): the nine sends into the headphones' two compressors, the
/// headphones' bus at -80 dB as the raid starts, the tinnitus' two sends
/// (`Tinnitus1`, `Tinnitus2`: found since the sends' levels are read).
/// The groups whose sends the cut goes by list no fader among their
/// effects (the fader after them all: the sends before it), and the
/// compressors' groups are at 0 dB. The headphones' fades from SPT's
/// client settings.
#[test]
fn the_mixer_s_sends_and_the_headphones_names_are_read() {
    use trp_data::audio::sound::{Exposed, FadeType};
    let Ok(install) = Install::locate() else {
        eprintln!("skipped: no install configured (set SPT_DIR or local/spt_dir.txt)");
        return;
    };
    let mixer = trp_data::audio::sound::master_mixer(&install).expect("the mixer");
    assert_eq!(mixer.start_snapshot, "Outdoor");
    let group = |name: &str| mixer.group(name).unwrap_or_else(|| panic!("no group {name}"));
    let name_of = |at: usize| mixer.groups[at].name.as_str();
    let send = |name: &str| match mixer.exposed(name) {
        Some(Exposed::Send { group, effect }) => {
            let e = &mixer.groups[group].effects[effect];
            (name_of(group), e.target.map_or("?", name_of), e.level.expect("a level").1)
        }
        other => panic!("{name}: {other:?}"),
    };
    let sends = [
        ("GunsCompressorSendLevel", "Guns", "GunCompressor", -15.0),
        ("ClientPlayerCompressorSendLevel", "ClientPlayer", "Compressor", -80.0),
        ("ObservedPlayerCompressorSendLevel", "ObservedPlayer", "Compressor", -80.0),
        ("NpcCompressorSendLevel", "NPC", "Compressor", -80.0),
        ("EnvTechnicalCompressorSendLevel", "TechnicalSounds", "Compressor", -80.0),
        ("EnvNatureCompressorSendLevel", "NatureSounds", "Compressor", -80.0),
        ("EnvCommonCompressorSendLevel", "CommonSounds", "Compressor", -80.0),
        ("AmbientCompressorSendLevel", "Ambient", "Compressor", -30.0),
        ("EffectsReturnsCompressorSendLevel", "Returns", "Compressor", -80.0),
        ("AmbientOccluded", "OccludedIn", "SimpleOccluded", -5.0),
        ("Tinnitus1", "Guns", "SimpleOccluded", -80.0),
        ("Tinnitus2", "Main", "SimpleOccluded", -80.0),
    ];
    for (name, from, to, level) in sends {
        assert_eq!(send(name), (from, to, level), "{name}");
    }
    let volumes = [
        ("HeadphonesMixerVolume", "Headphones", -80.0),
        ("OcclusionVolume", "SimpleOccluded", 0.0),
        ("EnvironmentVolume", "Environment", 0.0),
        ("AmbientVolume", "Ambient", 0.0),
        ("EffectsReturnsGroupVolume", "Returns", 0.0),
        ("GunsVolume", "Guns", 0.0),
        ("OutEnvironmentVolume", "OutEnvironment", 0.0),
    ];
    for (name, of, db) in volumes {
        assert_eq!(mixer.exposed(name), Some(Exposed::Volume { group: group(of) }), "{name}");
        assert!((mixer.value(name).unwrap() - db).abs() < 0.05, "{name}: {:?}", mixer.value(name));
    }
    assert_eq!(mixer.value("GunsCompressorSendLevel"), Some(-15.0));
    // The bus under `World`, the two compressors under it, both at 0 dB.
    assert_eq!(mixer.groups[group("Headphones")].parent, Some(group("World")));
    for compressor in ["GunCompressor", "Compressor"] {
        assert_eq!(mixer.groups[group(compressor)].parent, Some(group("Headphones")), "{compressor}");
        assert_eq!(mixer.groups[group(compressor)].volume_db, 0.0, "{compressor}");
    }
    // The sends before the faders: these groups list sends alone; the
    // returns' fader comes before its send.
    for sending in ["Guns", "Main", "Environment", "ClientPlayer", "ObservedPlayer", "NPC", "TechnicalSounds", "NatureSounds", "CommonSounds", "Ambient"] {
        let kinds: Vec<i64> = mixer.groups[group(sending)].effects.iter().map(|e| e.kind).collect();
        assert!(!kinds.is_empty() && kinds.iter().all(|kind| *kind == -3), "{sending}: {kinds:?}");
    }
    let returns: Vec<i64> = mixer.groups[group("Returns")].effects.iter().map(|e| e.kind).collect();
    assert_eq!(returns, [-2, -3]);
    let db = Database::load(&install).expect("database");
    let fades = trp_data::audio::sound::headphones_settings(&db.client);
    assert_eq!((fades.fade_duration, fades.fade_in, fades.fade_out), (0.7, FadeType::InverseExponential, FadeType::EaseIn));
}
