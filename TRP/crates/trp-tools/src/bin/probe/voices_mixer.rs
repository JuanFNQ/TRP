//! The voices, Vorbis clips and the master mixer.

use anyhow::Result;
use trp_data::unity::Assets;

// The things' sounds kept as Vorbis: each sample's channels,
// frequency, frames, packets and the CRC32 of its setup header,
// and whether that header was found in the game's engine.
pub fn voicecost(assets: &mut Assets) -> Result<()> {
    // What reading all of a voice's banks costs (the profile's
    // voice): the time with the clips' sounds and without, and the
    // sounds' size (what the gestures' phrases would hold).
    let name = anyhow::Context::context(assets.install.profile_voice(), "the profile names no voice")?;
    let every: Vec<i32> = (0..=400).collect();
    let started = std::time::Instant::now();
    let bare = trp_data::audio::sound::voice_banks(&assets.install, &name, &every, false)?;
    let bare_time = started.elapsed().as_secs_f32();
    let started = std::time::Instant::now();
    let whole = trp_data::audio::sound::voice_banks(&assets.install, &name, &every, true)?;
    let whole_time = started.elapsed().as_secs_f32();
    let clips: Vec<&trp_data::audio::sound::Clip> = whole.banks.iter().flat_map(|b| b.clips.iter().filter_map(|c| c.clip.as_ref())).collect();
    let bytes: usize = clips.iter().map(|c| c.samples.len() * 2).sum();
    let seconds: f32 = clips.iter().map(|c| c.samples.len() as f32 / c.channels.max(1) as f32 / c.frequency.max(1) as f32).sum();
    println!("{name}: {} banks, {} clips; without sounds {bare_time:.2} s, with {whole_time:.2} s; {} clips read, {seconds:.0} s of sound, {:.1} MB", bare.banks.len(), bare.banks.iter().map(|b| b.clips.len()).sum::<usize>(), clips.len(), bytes as f64 / 1e6);
    for bank in &whole.banks {
        let size: usize = bank.clips.iter().filter_map(|c| c.clip.as_ref()).map(|c| c.samples.len() * 2).sum();
        println!("  trigger {} importance {} ignore tags {}: {} clips, {:.2} MB", bank.trigger, bank.importance, bank.ignore_tags, bank.clips.len(), size as f64 / 1e6);
    }
    Ok(())
}

pub fn voice(assets: &mut Assets, args: &[String]) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    // A voice's health banks (the profile's, or the one named), each
    // clip with its tags, volume, falloff, length and level; with a
    // folder, each written there as a WAVE file.
    let name = match arg(1).filter(|a| !a.contains(['/', '\\'])) {
        Some(name) => name.to_owned(),
        None => anyhow::Context::context(assets.install.profile_voice(), "the profile names no voice")?,
    };
    let folder = arg(1).filter(|a| a.contains(['/', '\\'])).or(arg(2)).map(std::path::PathBuf::from);
    let voice = trp_data::audio::sound::voice_banks(&assets.install, &name, &[9, 15, 16, 26, 27, 29], true)?;
    println!("{} from {}: {} banks", voice.name, voice.bundle, voice.banks.len());
    for bank in &voice.banks {
        let chain = bank.chain;
        println!("trigger {} importance {} ignore tags {} chain (who {}, phrase {}, {}%): {} clips", bank.trigger, bank.importance, bank.ignore_tags, chain.who, chain.event, chain.probability, bank.clips.len());
        for c in &bank.clips {
            let (rms, peak) = c.clip.as_ref().map_or((f64::NAN, f64::NAN), |clip| {
                let sum: f64 = clip.samples.iter().map(|s| (*s as f64) * (*s as f64)).sum();
                let rms = (sum / clip.samples.len().max(1) as f64).sqrt() / 32768.0;
                let peak = clip.samples.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0) as f64 / 32768.0;
                (20.0 * rms.max(1e-9).log10(), 20.0 * peak.max(1e-9).log10())
            });
            let read = c.clip.as_ref().map_or("not read".to_owned(), |clip| format!("{} Hz x{}, {:.3} s", clip.frequency, clip.channels, clip.samples.len() as f32 / clip.channels.max(1) as f32 / clip.frequency.max(1) as f32));
            println!("  {}: mask {:#06x}, volume {}, falloff {}, length {:.3} ({read}), rms {rms:.1} dBFS, peak {peak:.1} dBFS", c.name, c.mask, c.volume, c.falloff, c.length);
            if let (Some(folder), Some(clip)) = (&folder, &c.clip) {
                std::fs::create_dir_all(folder)?;
                std::fs::write(folder.join(format!("{}_{}.wav", bank.trigger, c.name)), clip.wav())?;
            }
        }
    }
    if let Ok(clip) = trp_data::audio::sound::fracture_sound(&assets.install) {
        println!("fracture: {} {} Hz x{}, {:.3} s", clip.name, clip.frequency, clip.channels, clip.samples.len() as f32 / clip.channels.max(1) as f32 / clip.frequency.max(1) as f32);
    }
    Ok(())
}

pub fn vorbis(assets: &mut Assets, args: &[String]) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let clips = trp_data::audio::sound::item_vorbis(&assets.install)?;
    let wanted: Vec<u32> = clips.iter().map(|(_, sample)| sample.setup_crc).collect();
    let headers = trp_data::audio::sound::vorbis_setup_headers(&assets.install, &wanted)?;
    // With a folder, each decoded and written there as a WAVE file.
    let folder = arg(1).map(std::path::PathBuf::from);
    for (name, sample) in &clips {
        let found = headers.get(&sample.setup_crc).map_or("not found".to_owned(), |header| format!("found, {} bytes", header.len()));
        println!("{name}: {} Hz x{}, {} frames, {} packets ({} bytes), setup header {:08x} {found}", sample.frequency, sample.channels, sample.frames, sample.packets.len(), sample.packets.iter().map(Vec::len).sum::<usize>(), sample.setup_crc);
        let (Some(folder), Some(setup)) = (&folder, headers.get(&sample.setup_crc)) else { continue };
        match trp_data::audio::sound::decode_vorbis(sample, setup) {
            Ok(samples) => {
                let ch = sample.channels.max(1) as usize;
                let peak = samples.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0);
                let level: f64 = samples.iter().map(|s| (*s as f64).abs()).sum::<f64>() / samples.len().max(1) as f64;
                let jumps: f64 = samples.windows(ch + 1).map(|w| (w[ch] as f64 - w[0] as f64).abs()).sum::<f64>() / samples.len().max(1) as f64;
                println!("  decoded: {} frames of {}, peak {peak}, roughness {:.2}", samples.len() / ch, sample.frames, jumps / level.max(1.0));
                std::fs::create_dir_all(folder)?;
                let clip = trp_data::audio::sound::Clip { name: name.clone(), channels: sample.channels, frequency: sample.frequency, samples };
                std::fs::write(folder.join(format!("{name}.wav")), clip.wav())?;
            }
            Err(e) => println!("  not decoded: {e:#}"),
        }
    }
    Ok(())
}

// The game's audio mixer (`mixer`, the bundle
// `assets/content/audio/mastermixer.bundle`): its groups as a tree,
// each with its volume in each snapshot (dB, from the snapshot's
// values at the group's `volumeIndex`), whether a script may set it
// (an exposed parameter), and the kinds of its effects, a send's
// with its target and level (`-3>Compressor@-80`).
pub fn mixer(assets: &mut Assets) -> Result<()> {
    let mixer = trp_data::audio::sound::master_mixer(&assets.install)?;
    println!("start snapshot: {}", mixer.start_snapshot);
    fn walk(mixer: &trp_data::audio::sound::Mixer, at: usize, depth: usize) {
        let group = &mixer.groups[at];
        let effects: Vec<String> = group
            .effects
            .iter()
            .map(|e| match (e.target, e.level) {
                (Some(target), Some((_, level))) => format!("{}>{}@{level}", e.kind, mixer.groups[target].name),
                _ if e.parameters.is_empty() => e.kind.to_string(),
                _ => format!("{}[{}]", e.kind, e.parameters.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(" ")),
            })
            .collect();
        println!("{}{}: {:+.1} dB{} (path {:+.1} dB); effects {}", "  ".repeat(depth), group.name, group.volume_db, if group.exposed { " exposed" } else { "" }, mixer.path_db(at), effects.join(" "));
        for (child, g) in mixer.groups.iter().enumerate() {
            if g.parent == Some(at) {
                walk(mixer, child, depth + 1);
            }
        }
    }
    for (at, group) in mixer.groups.iter().enumerate() {
        if group.parent.is_none() {
            walk(&mixer, at, 0);
        }
    }
    println!("the master's compressor: {:?}", mixer.master_compressor());
    println!("{} exposed parameters", mixer.exposed.len());
    // The names the game sets (`AudioMixerDataContainer`, then the
    // headphones' templates', `ActiveHeadphonesDefaultConfigurator`),
    // or those asked.
    let asked: Vec<String> = std::env::args().skip(2).collect();
    let names: Vec<String> = if asked.is_empty() {
        [
            "GunsVolume",
            "MainVolume",
            "InGame",
            "WorldVolume",
            "WorldLowpassFreq",
            "Tinnitus1",
            "Tinnitus2",
            "ReverbVolume",
            "OutEnvironmentVolume",
            "MasterVolume",
            "UIVolume",
            "GunsCompressorSendLevel",
            "ClientPlayerCompressorSendLevel",
            "ObservedPlayerCompressorSendLevel",
            "NpcCompressorSendLevel",
            "EnvTechnicalCompressorSendLevel",
            "EnvNatureCompressorSendLevel",
            "EnvCommonCompressorSendLevel",
            "AmbientCompressorSendLevel",
            "EffectsReturnsCompressorSendLevel",
            "HeadphonesMixerVolume",
            "OcclusionVolume",
            "EnvironmentVolume",
            "AmbientVolume",
            "AmbientOccluded",
            "EffectsReturnsGroupVolume",
        ]
        .map(str::to_owned)
        .to_vec()
    } else {
        asked
    };
    for name in names {
        match mixer.exposed(&name) {
            Some(trp_data::audio::sound::Exposed::Volume { group }) => println!("  {name}: the volume of {}", mixer.groups[group].name),
            Some(trp_data::audio::sound::Exposed::Send { group, effect }) => {
                let e = &mixer.groups[group].effects[effect];
                let target = e.target.map_or("?".to_owned(), |t| mixer.groups[t].name.clone());
                println!("  {name}: the send of {} to {target}, at {:?} dB", mixer.groups[group].name, e.level.map(|(_, level)| level));
            }
            Some(trp_data::audio::sound::Exposed::Effect { group, effect, parameter }) => {
                let e = &mixer.groups[group].effects[effect];
                let target = e.target.map(|t| mixer.groups[t].name.clone());
                println!("  {name}: parameter {parameter} ({:?}) of effect {effect} (kind {}) of {}, sending to {target:?}", e.parameters.get(parameter), e.kind, mixer.groups[group].name);
            }
            None => println!("  {name}: not found"),
        }
    }
    Ok(())
}
