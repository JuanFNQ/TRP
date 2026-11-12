//! The sounds of the interface, things, drops, steps, hands and aiming.

use anyhow::Result;
use trp_data::unity::Assets;

// The interface's sounds (`Resources/Audio/UISoundsWrapper`): each
// sound's kind and its clip's name, length and how it is kept.
pub fn uisounds(assets: &mut Assets) -> Result<()> {
    let paths = trp_data::interface::screen::resource_paths(&assets.install)?;
    let (_, file, id) = paths.iter().find(|(path, ..)| path == "audio/uisoundswrapper").ok_or_else(|| anyhow::anyhow!("no audio/uisoundswrapper"))?;
    let (class, bytes) = trp_data::interface::screen::object_bytes(&assets.install, file, *id)?;
    println!("{file} {id}: class {class}, {} bytes", bytes.len());
    let tree = trp_data::interface::screen::tree_of(&assets.install, "assets/content/audio/itemsounds/itemsounds.bundle", 83)?;
    let mut pictures = trp_data::interface::screen::ScenePictures::new(&assets.install)?;
    // After the object, its switch, its script and its name: the
    // list of kinds and clips.
    let mut at = 12 + 4 + 12;
    let name_len = u32::from_le_bytes(bytes[at..at + 4].try_into()?) as usize;
    at += 4 + name_len;
    at = (at + 3) & !3;
    let count = u32::from_le_bytes(bytes[at..at + 4].try_into()?) as usize;
    at += 4;
    for n in 0..count {
        let kind = i32::from_le_bytes(bytes[at + 16 * n..at + 16 * n + 4].try_into()?);
        let clip_file = i32::from_le_bytes(bytes[at + 16 * n + 4..at + 16 * n + 8].try_into()?);
        let clip_id = i64::from_le_bytes(bytes[at + 16 * n + 8..at + 16 * n + 16].try_into()?);
        let said = if clip_file == 0 { pictures.read_by(file, clip_id, 83, &tree).map(|v| format!("{:?} {:?}s format {:?} {:?}", v.get("m_Name").and_then(|n| n.as_str()).unwrap_or(""), v.get("m_Length").and_then(|l| l.as_f32()), v.get("m_CompressionFormat").and_then(|f| f.as_i64()), v.get("m_Resource").and_then(|r| r.get("m_Source")).and_then(|s| s.as_str()))).unwrap_or_else(|e| format!("{e:#}")) } else { format!("in file {clip_file}") };
        println!("  kind {kind}: {said}");
    }
    Ok(())
}

// The sounds of moving (`sound::movement_sounds`): each surface's
// banks with their figures and how many clips each list has; with
// `read`, every clip decoded too and a count of those not read; with
// a folder after `read`, the clips of the surfaces given (by number,
// `stepsounds read local\x 0 2`) written there as WAVE files.
// The things' sounds of a map's scenes (`thingsounds [level8
// level10 ...]`; the app's two unless told): every door, container
// and trunk with its object's path, the clips its script names for
// its opening, its squeak and its shutting with their rolloffs, its
// shut shift and its angles, whether it has a handle, and a door's
// breach and hit clips; then every clip named, once, with how it is
// kept. `thingsounds read <folder> [levels]` decodes the clips and
// writes them out as WAVE files.
pub fn thingsounds(assets: &mut Assets, args: &[String], first: Option<&str>) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let read = first.as_deref() == Some("read");
    let from = if read { 3 } else { 2 };
    let levels: Vec<&str> = (from..16).filter_map(|n| arg(n)).collect();
    let levels = if levels.is_empty() { trp_data::audio::thing_sound::SCENES.to_vec() } else { levels };
    let sounds = trp_data::audio::thing_sound::thing_sounds(&assets.install, &levels, read)?;
    let names = |list: &[usize]| list.iter().map(|&at| sounds.clip_name(at).to_owned()).collect::<Vec<_>>().join(", ");
    for set in &sounds.sets {
        println!("{} {} [{}] {}", set.scene, set.kind.name(), set.name, set.path);
        println!("  open [{}] rolloff {}; squeak [{}] rolloff {}; shut [{}] rolloff {}", names(&set.open), set.rolloffs[0], names(&set.squeak), set.rolloffs[1], names(&set.shut), set.rolloffs[2]);
        println!("  shut shift {}, angles {:?}, handle {}", set.shut_shift, set.angles, set.has_handle);
        if set.kind == trp_data::audio::thing_sound::ThingKind::Door {
            println!("  breach [{}] rolloff {}; hit [{}] rolloff {}", set.breach.map_or("", |at| sounds.clip_name(at)), set.breach_rolloff, set.hit.map_or("", |at| sounds.clip_name(at)), set.hit_rolloff);
        }
    }
    println!("{} things, {} clips named", sounds.sets.len(), sounds.clips.len());
    for (name, clip) in &sounds.clips {
        match clip {
            Some(clip) => println!("  {name}: {} channels at {} Hz, {:.3} s", clip.channels, clip.frequency, clip.samples.len() as f32 / clip.channels.max(1) as f32 / clip.frequency.max(1) as f32),
            None => println!("  {name}"),
        }
    }
    if let Some(folder) = arg(2).filter(|_| read) {
        let folder = std::path::Path::new(folder);
        std::fs::create_dir_all(folder)?;
        for (name, clip) in &sounds.clips {
            if let Some(clip) = clip {
                std::fs::write(folder.join(format!("{name}.wav")), clip.wav())?;
            }
        }
        println!("written to {}", folder.display());
    }
    Ok(())
}

pub fn dropsounds(assets: &mut Assets, read: Option<&str>) -> Result<()> {
    // The weapons' landing sounds: the energy's curve, the default
    // surface, each surface's three banks.
    let sounds = trp_data::audio::sound::drop_sounds(&assets.install, read.as_deref() == Some("read"))?;
    let name = |surface: u8| trp_data::audio::sound::SURFACES.get(surface as usize).copied().unwrap_or("?");
    println!("volume by energy: {:?}", sounds.volume.keys.iter().map(|k| (k.time, k.value, k.in_slope, k.out_slope)).collect::<Vec<_>>());
    for energy in [0.0f32, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0] {
        print!("  {energy}: {:.2}", sounds.volume.evaluate(energy));
    }
    println!();
    println!("default surface {} ({})", name(sounds.default_surface), sounds.default_surface);
    for (surface, banks) in &sounds.surfaces {
        let said: Vec<String> = trp_data::audio::sound::DROP_KINDS
            .iter()
            .zip(banks)
            .map(|(kind, bank)| match bank {
                Some(at) => {
                    let bank = &sounds.banks[*at];
                    format!("{kind}: {} volume {:.2} -{:.2} pitch {:.2} +-{:.2} rolloff {:.0} blend {:?} clips {:?} source {}", bank.name, bank.base_volume, bank.delta_volume, bank.base_pitch, bank.delta_pitch, bank.rolloff, bank.blend, bank.lists.iter().map(Vec::len).collect::<Vec<_>>(), bank.source_type)
                }
                None => format!("{kind}: none"),
            })
            .collect();
        println!("{} ({surface}):\n  {}", name(*surface), said.join("\n  "));
    }
    let decoded = sounds.clips.iter().filter(|(_, clip)| clip.is_some()).count();
    println!("{} banks, {} clips named, {decoded} read", sounds.banks.len(), sounds.clips.len());
    Ok(())
}

pub fn stepsounds(assets: &mut Assets, args: &[String], read: Option<&str>) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let read = read.as_deref() == Some("read");
    let only: Vec<u8> = (3..16).filter_map(|n| arg(n).and_then(|s| s.parse().ok())).collect();
    let sounds = trp_data::audio::sound::movement_sounds(&assets.install, (!only.is_empty()).then_some(only.as_slice()), read)?;
    let say = |bank: &trp_data::audio::sound::Bank| {
        format!(
            "{:28} volume {:.2} -{:.2}, pitch {:.2} +-{:.2}, rolloff {:5.1}, blend {:?}, clips {:?}, source {}",
            bank.name,
            bank.base_volume,
            bank.delta_volume,
            bank.base_pitch,
            bank.delta_pitch,
            bank.rolloff,
            bank.blend,
            bank.lists.iter().map(Vec::len).collect::<Vec<_>>(),
            bank.source_type
        )
    };
    for set in &sounds.surfaces {
        println!("{} ({})", trp_data::audio::sound::SURFACES.get(set.surface as usize).unwrap_or(&"?"), set.surface);
        for (name, bank) in trp_data::audio::sound::SURFACE_BANKS.iter().zip(&set.banks) {
            println!("  {name:20} {}", bank.map_or("none".to_owned(), |bank| say(&sounds.banks[bank])));
        }
    }
    for (name, bank) in trp_data::audio::sound::GEAR_BANKS.iter().zip(&sounds.gear) {
        println!("{name:22} {}", bank.map_or("none".to_owned(), |bank| say(&sounds.banks[bank])));
    }
    // The vault's: each surface's elements by move and mark, and
    // the three moves' kit banks.
    let banks = &sounds.banks;
    for set in &sounds.vault {
        let listed: Vec<String> = trp_data::audio::sound::VAULT_SETS
            .iter()
            .zip(&set.elements)
            .flat_map(|(kind, marks)| {
                trp_data::audio::sound::VAULT_MARKS.iter().zip(marks).filter_map(move |(mark, element)| {
                    let element = (*element)?;
                    let bank = element.bank.map_or("no bank".to_owned(), |bank| {
                        let bank = &banks[bank];
                        format!("{} (volume {:.2} -{:.2}, clips {:?}, blend {:?})", bank.name, bank.base_volume, bank.delta_volume, bank.lists.iter().map(Vec::len).collect::<Vec<_>>(), bank.blend)
                    });
                    Some(format!("{kind}.{mark}: {bank} at {:.2}-{:.2}", element.min_volume, element.max_volume))
                })
            })
            .collect();
        println!("{} ({}) vault: {}", trp_data::audio::sound::SURFACES.get(set.surface as usize).unwrap_or(&"?"), set.surface, if listed.is_empty() { "nothing".to_owned() } else { listed.join("; ") });
    }
    for (name, bank) in trp_data::audio::sound::VAULT_GEAR_BANKS.iter().zip(&sounds.vault_gear) {
        println!("{name:22} {}", bank.map_or("none".to_owned(), |bank| say(&sounds.banks[bank])));
    }
    // A switch on the head's clips and the tactical sound, each its
    // name in the `Sounds` and the clip's own.
    for (name, clip, _) in &sounds.toggles {
        println!("{name:22} {clip}");
    }
    let mut kinds: std::collections::BTreeMap<(u16, u32), (usize, f32)> = std::collections::BTreeMap::new();
    for clip in sounds.clips.iter().filter_map(|(_, clip)| clip.as_ref()) {
        let entry = kinds.entry((clip.channels, clip.frequency)).or_default();
        entry.0 += 1;
        entry.1 = entry.1.max(clip.samples.len() as f32 / clip.channels.max(1) as f32 / clip.frequency.max(1) as f32);
    }
    for ((channels, rate), (count, longest)) in &kinds {
        println!("  {count} clips of {channels} channels at {rate} Hz, the longest {longest:.2} s");
    }
    let decoded = sounds.clips.iter().filter(|(_, clip)| clip.is_some()).count();
    let frames: usize = sounds.clips.iter().filter_map(|(_, clip)| clip.as_ref()).map(|clip| clip.samples.len()).sum();
    println!("{} clips named, {decoded} read ({:.1} MB of 16-bit samples)", sounds.clips.len(), frames as f64 * 2.0 / 1e6);
    if let Some(folder) = arg(2).filter(|_| read) {
        let folder = std::path::Path::new(folder);
        std::fs::create_dir_all(folder)?;
        for (name, clip) in &sounds.clips {
            if let Some(clip) = clip {
                std::fs::write(folder.join(format!("{name}.wav")), clip.wav())?;
            }
        }
    }
    Ok(())
}

// A part's scripts as read (`bipodaudio <template> [script]`): the
// prefab's MonoBehaviours of that script (`BipodAudioController`
// unless told), each value whole.
pub fn bipodaudio(assets: &mut Assets, args: &[String], template: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let db = trp_data::Database::load(&assets.install)?;
    let item = db.items.get(template).ok_or_else(|| anyhow::anyhow!("no such template"))?;
    let key = item.prefab_key().ok_or_else(|| anyhow::anyhow!("no prefab"))?;
    let wanted = arg(2).unwrap_or("BipodAudioController");
    let files = assets.load(key)?;
    for &file in &files {
        for obj in assets.objects_of(file, trp_data::unity::class::MONO_BEHAVIOUR).collect::<Vec<_>>() {
            if trp_data::unity::extract::script_class(&assets, obj).as_deref() == Some(wanted) {
                println!("{wanted} in {key} (file {file}):\n{:#?}", assets.read(obj)?);
            }
        }
    }
    Ok(())
}

// A hands rig's sound player (`BaseSoundPlayer`): each of its
// events with its rolloff, volume and clips. An item that has
// hands of its own by its template (its `UsePrefab`), else a weapon.
pub fn handsounds(assets: &mut Assets, args: &[String], what: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let db = trp_data::Database::load(&assets.install)?;
    let key = db.items.get(what).and_then(|item| item.props["UsePrefab"]["path"].as_str().filter(|path| !path.is_empty()).map(str::to_owned)).unwrap_or_else(|| what.to_owned());
    let weapon = trp_data::weapons::weapon::assemble_named(&mut *assets, &db, &key)?;
    let mut setups = trp_data::audio::sound::VorbisSetups::new(&assets.install);
    match trp_data::audio::sound::hands_sounds(&assets, &weapon, &mut setups, true)? {
        Some(sounds) => {
            println!("{key}: {}", sounds.player);
            for (list, elements) in [("main", &sounds.main), ("additional", &sounds.additional)] {
                for element in elements {
                    let clips: Vec<String> = element.clips.iter().map(|clip| format!("{} ({:.2} s, x{})", clip.name, clip.samples.len() as f32 / clip.channels.max(1) as f32 / clip.frequency.max(1) as f32, clip.channels)).collect();
                    println!("  {list} {}: rolloff {}, volume {}, {}", element.event, element.rolloff, element.volume, clips.join(", "));
                }
            }
            // Its shots (`WeaponSoundPlayer`): each bank's figures
            // and its three lists by distance.
            if let Some(shots) = &sounds.shots {
                println!("  shots: {}", if shots.non_auto { "each whole (Non_auto)" } else { "automatic: a loop and a tail" });
                for (name, bank) in trp_data::audio::sound::SHOT_BANKS.iter().zip(&shots.banks) {
                    let Some(bank) = bank else {
                        println!("  {name}: none");
                        continue;
                    };
                    println!("  {name} = {}: volume {}, rolloff {}, length {:.4} s, blend {:?}", bank.name, bank.base_volume, bank.rolloff, bank.clip_length, bank.blend);
                    for (n, list) in bank.lists.iter().enumerate() {
                        let clips: Vec<String> = list
                            .iter()
                            .map(|&at| match &shots.clips[at] {
                                (name, Some(clip)) => format!("{name} ({:.4} s, x{}, {} Hz)", clip.samples.len() as f32 / clip.channels.max(1) as f32 / clip.frequency.max(1) as f32, clip.channels, clip.frequency),
                                (name, None) => format!("{name} (not read)"),
                            })
                            .collect();
                        println!("    distance {n}: {}", clips.join(", "));
                    }
                }
                let bytes: usize = shots.clips.iter().filter_map(|(_, clip)| clip.as_ref()).map(|clip| clip.samples.len() * 2).sum();
                println!("  {} clips, {:.1} MB decoded", shots.clips.len(), bytes as f32 / 1e6);
                // With a folder (`handsounds <weapon> <folder>`), each
                // clip written there as a WAVE file, with its peak.
                if let Some(folder) = arg(2) {
                    let folder = std::path::Path::new(folder);
                    std::fs::create_dir_all(folder)?;
                    for clip in shots.clips.iter().filter_map(|(_, clip)| clip.as_ref()) {
                        let peak = clip.samples.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0);
                        let rms = (clip.samples.iter().map(|&s| (s as f64).powi(2)).sum::<f64>() / clip.samples.len().max(1) as f64).sqrt();
                        println!("  {}: peak {peak}, rms {rms:.0}", clip.name);
                        std::fs::write(folder.join(format!("{}.wav", clip.name)), clip.wav())?;
                    }
                }
            }
        }
        None => println!("{key}: no sound player"),
    }
    Ok(())
}

pub fn sounds(assets: &mut Assets, args: &[String], folder: &str) -> Result<()> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let folder = std::path::Path::new(folder);
    std::fs::create_dir_all(folder)?;
    let ui = trp_data::audio::sound::ui_sounds(&assets.install, &(0..64).collect::<Vec<i32>>())?;
    let items = trp_data::audio::sound::item_sounds(&assets.install)?;
    let group = arg(2).unwrap_or("weap_ar");
    let mut all: Vec<(String, &trp_data::audio::sound::Clip)> = ui.by_kind.iter().map(|(kind, clip)| (format!("ui{kind}"), clip)).collect();
    all.extend(ui.load.iter().enumerate().map(|(n, clip)| (format!("load{n}"), clip)));
    all.extend(ui.unload.iter().enumerate().map(|(n, clip)| (format!("unload{n}"), clip)));
    all.extend(items.iter().filter(|(name, _)| name.starts_with(group)).map(|(name, clip)| (name.clone(), clip)));
    for (name, clip) in &all {
        std::fs::write(folder.join(format!("{name}_{}.wav", clip.name)), clip.wav())?;
        let ch = clip.channels as usize;
        let level: f64 = clip.samples.iter().map(|s| (*s as f64).abs()).sum::<f64>() / clip.samples.len().max(1) as f64;
        let jumps: f64 = clip.samples.windows(ch + 1).map(|w| (w[ch] as f64 - w[0] as f64).abs()).sum::<f64>() / clip.samples.len().max(1) as f64;
        println!("{name}: {} {} Hz x{}, {:.3} s, peak {}, roughness {:.2}", clip.name, clip.frequency, clip.channels, clip.samples.len() as f32 / clip.channels as f32 / clip.frequency as f32, clip.samples.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0), jumps / level.max(1.0));
    }
    println!("the things' sounds: {} clips", items.len());
    Ok(())
}

// The aiming's sounds (`aimsounds`): each kind of weapon's bank,
// its volume, rolloff and clips.
pub fn aimsounds(assets: &mut Assets) -> Result<()> {
    for (kind, bank) in trp_data::audio::sound::aiming_sounds(&assets.install)? {
        let clips: Vec<String> = bank.clips.iter().map(|clip| format!("{} ({:.2} s, x{})", clip.name, clip.samples.len() as f32 / clip.channels.max(1) as f32 / clip.frequency.max(1) as f32, clip.channels)).collect();
        println!("{kind}: {}, volume {}, rolloff {}: {}", bank.name, bank.volume, bank.rolloff, clips.join(", "));
    }
    Ok(())
}
