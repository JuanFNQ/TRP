//! The engine's audio group presets (`AudioGroupPreset`): what each group of
//! the game's sound sources (`BetterAudio.AudioSourceGroupType`: gunshots,
//! the character, collisions, the environment and the rest) is made with.
//!
//! `BetterAudio` loads one preset a group from the build's own Resources by
//! its name (`LoadGroupPreset`: `Audio/AudioGroupPresets/` and the group's
//! name, as `Character`, `Collisions`, `Environment`), then lays the
//! backend's figures over it (`ClientAudioSettings`, the `AudioGroupPresets`
//! of the install's `settings.json`: the overall volume and the binaural
//! limits, by the group's number; `apply_client_settings` here). Every
//! source of a group is made from its preset (`NonspatialAudioMixer`
//! `.InstantiateNewSource`): the engine's source is set to a custom rolloff
//! whose curve is the preset's `SoundRolloff`, its spread curve to
//! `SpreadCurve`, its spatial blend to `SpatialBlend`, its priority to
//! `AudioSourcePriority`, and its rolloff distance to `DefaultMaxDistance`
//! (`SetRolloff`, which makes the engine source's maximum distance that
//! distance times the source's occlusion rolloff scale, which is one
//! unless the occlusion system, `SpatialAudioOccluder`, has set it). A sound placed in the world sets the distance again each time
//! it is played: `BetterAudio.PlayAtPoint` with a clip is given a rolloff
//! in whole metres (a door's or a container's `_openSoundRolloff` and the
//! like, in the `InteractiveObjects` group), plays nothing when the
//! listener is farther than that, and else sets the source's rolloff to it;
//! a bank's sound does the same with the bank's `Rolloff`. The base volume
//! a source is given is multiplied by `OverallVolume`
//! (`BetterSource.SetBaseVolume`). A source made over a prefab's own
//! (`CreateBetterSource`) is given the two curves, but only the variant
//! that keeps the prefab's settings (`CreateBetterSourceWithParentSettings`)
//! sets its rolloff to the custom one, and with the prefab's own curve.
//!
//! The engine evaluates a custom rolloff curve at the listener's distance
//! as a share of the source's maximum distance, so the curves' times run
//! from nought to one (every preset of the install does, `Character`'s to
//! 1.0015, and each ends at nought); that is Unity's rule, not read from
//! the game's code. So a sound's gain at a distance is the curve at the
//! distance over the rolloff it was played with (`rolloff_gain`).
//!
//! The presets lie in `resources.assets`, a serialized file with no type
//! trees, so each is read by a layout written from the class's fields in
//! their declared order: the nested settings classes inline
//! (`AudioGroupAcousticSettings`, `AudioGroupOcclusionSettings` with its
//! two `AudioOcclusionEQPreset`s), enums as four-byte numbers, and every
//! flag padded to four bytes. A preset is taken only when the layout uses
//! its object's bytes exactly, to the last, which all seventeen do.

use anyhow::{bail, ensure, Context, Result};

use crate::curve::{Curve, Key};
use crate::install::Install;
use crate::unity::class;
use crate::unity::reader::Reader;

/// Where `BetterAudio.LoadGroupPreset` loads the presets from, as the
/// build's resource table keeps it (lower case).
pub const RESOURCE_DIR: &str = "audio/audiogrouppresets/";

/// The groups' names by their numbers (`BetterAudio.AudioSourceGroupType`).
pub const GROUP_NAMES: [&str; 19] = [
    "Gunshots",
    "Weaponry",
    "Impacts",
    "Character",
    "Environment",
    "Collisions",
    "Speech",
    "Distant",
    "NonspatialBypass",
    "Nonspatial",
    "Voip",
    "Grenades",
    "Windows",
    "VehicleInSpeech",
    "Lamp",
    "InteractiveObjects",
    "OutEnvironment",
    "PropagatedExclusive",
    "LightOcclusion",
];

/// A group's name by its number, or an empty name for a number the enum
/// does not have.
pub fn group_name(group: i32) -> &'static str {
    usize::try_from(group).ok().and_then(|i| GROUP_NAMES.get(i)).copied().unwrap_or("")
}

/// One preset as the install keeps it.
#[derive(Debug, Clone)]
pub struct AudioGroupPreset {
    /// The object's name, which is the name it is loaded by (`Character`).
    pub name: String,
    /// The path the build's resource table has it at.
    pub resource_path: String,
    /// The group it is for (`Type`, `AudioSourceGroupType`; `group_name`).
    pub group: i32,
    /// A factor on every base volume a source of the group is given.
    pub overall_volume: f32,
    /// The sources' spatial blend: nought flat, one wholly placed.
    pub spatial_blend: f32,
    pub steam_spatialize: bool,
    pub direct_binaural: bool,
    pub disable_binaural_by_dist: bool,
    pub enable_binaural_dist: f32,
    pub height_diff_to_allow_binaural: f32,
    pub angle_to_allow_binaural: f32,
    pub hrtf_intensity: f32,
    /// `DirectivityPatternType`, the spatializer's (Meta XR's) enum.
    pub directivity_pattern: i32,
    pub directivity_intensity: f32,
    pub occlusion_source_radius: f32,
    /// The volume by distance (`SoundRolloff`): the time a share of the
    /// source's maximum distance, the value a gain.
    pub sound_rolloff: Curve,
    /// Whether any key of `sound_rolloff` is weighted, which `Curve` does
    /// not evaluate.
    pub sound_rolloff_weighted: bool,
    /// The spread by distance (`SpreadCurve`), on the same axis.
    pub spread_curve: Curve,
    /// The engine source's priority (`AudioSourcePriority`).
    pub priority: i32,
    pub pre_cached_sources: i32,
    pub sources_limit: i32,
    /// The rolloff distance a source of the group starts with, in metres
    /// (`DefaultMaxDistance`), until a bank sets its own.
    pub default_max_distance: f32,
    pub acoustic: AcousticSettings,
    pub occlusion: OcclusionSettings,
}

/// The reverb sends (`AudioGroupPreset.AcousticSettings`).
#[derive(Debug, Clone, Copy)]
pub struct Reverb {
    pub enabled: bool,
    pub reverb_send_db: f32,
    pub early_reflections_send_db: f32,
    pub reverb_reach: f32,
}

/// `AudioGroupAcousticSettings`: the reverb of a mono and of a stereo
/// source.
#[derive(Debug, Clone, Copy)]
pub struct AcousticSettings {
    pub mono: Reverb,
    pub stereo: Reverb,
    pub prewarm: bool,
}

/// `AudioGroupOcclusionSettings`.
#[derive(Debug, Clone)]
pub struct OcclusionSettings {
    pub enabled: bool,
    pub intensity: f32,
    pub max_quality_factor: f32,
    pub use_quality_compression: bool,
    pub indoor_to_outdoor: f32,
    pub outdoor_to_indoor: f32,
    /// `rolloffScale`.
    pub rolloff_scale: f32,
    pub obstruction: EqPreset,
    pub propagation: EqPreset,
    pub stairs_height_curve: Curve,
}

/// `AudioOcclusionEQPreset`.
#[derive(Debug, Clone)]
pub struct EqPreset {
    pub volume_curve: Curve,
    /// `environmentVolumeThresholds`: indoor isolated, different rooms'
    /// types isolated, different environment isolated, indoor to outdoor,
    /// outdoor to indoor, the base.
    pub environment_volume_thresholds: [f32; 6],
    pub height_volume_curve: Curve,
    pub high_pass: FilterSettings,
    pub low_pass: FilterSettings,
    pub distance_coefficient: f32,
    pub rotation_coefficient: f32,
}

/// `AudioOcclusionEQPreset.AudioFilterSettings`.
#[derive(Debug, Clone)]
pub struct FilterSettings {
    pub frequency_curve: Curve,
    pub resonance_curve: Curve,
    pub distance_curve: Curve,
    pub height_curve: Curve,
    /// `positionEqThresholds`: level, above, below, behind (hertz).
    pub position_thresholds: [f32; 4],
    /// `environmentEqThresholds`, in the order of the volume's.
    pub environment_thresholds: [f32; 6],
}

impl AudioGroupPreset {
    /// The gain of the preset's rolloff at a distance from a source whose
    /// maximum distance is `max_distance` (metres both).
    pub fn rolloff_gain(&self, distance: f32, max_distance: f32) -> f32 {
        if max_distance <= 0.0 {
            return self.sound_rolloff.evaluate(0.0);
        }
        self.sound_rolloff.evaluate(distance / max_distance)
    }
}

/// Every preset in the build's Resources under `RESOURCE_DIR`, in the
/// table's order, as the install keeps them (before the backend's figures:
/// `apply_client_settings`).
pub fn audio_group_presets(install: &Install) -> Result<Vec<AudioGroupPreset>> {
    let mut wanted: Vec<(String, String, i64)> =
        crate::screen::resource_paths(install)?.into_iter().filter(|(path, ..)| path.starts_with(RESOURCE_DIR)).collect();
    ensure!(!wanted.is_empty(), "the build's resources have nothing under {RESOURCE_DIR}");
    wanted.sort_by(|a, b| a.1.cmp(&b.1));
    let mut presets = Vec::with_capacity(wanted.len());
    let mut open: Option<(String, crate::screen::BuildFile)> = None;
    for (path, file, id) in wanted {
        if open.as_ref().map_or(true, |(name, _)| *name != file) {
            open = Some((file.clone(), crate::screen::BuildFile::open(install, &file)?));
        }
        let (_, build) = open.as_mut().expect("opened above");
        let object = *build.meta.object(id).with_context(|| format!("{path}: no object {id} in {file}"))?;
        // Beside each preset lies a text of its occlusion's settings
        // (`<name>_occlusionsettings`), which no code of the game's loads.
        if object.class_id != class::MONO_BEHAVIOUR {
            continue;
        }
        let bytes = build.bytes(object.offset, object.size)?;
        let big_endian = build.meta.big_endian;
        let preset = parse(&bytes, big_endian, path.clone()).with_context(|| format!("{path} ({file}, object {id})"))?;
        presets.push(preset);
    }
    Ok(presets)
}

/// Lays the backend's figures over the presets as `LoadGroupPreset` does:
/// `DirectBinaural` on for every one, and where the client settings'
/// `AudioSettings.AudioGroupPresets` have the group's number, its overall
/// volume and binaural limits. `client` is the `config` of `settings.json`
/// (`Database::client`). Returns how many presets were given figures.
pub fn apply_client_settings(presets: &mut [AudioGroupPreset], client: &serde_json::Value) -> usize {
    let backend = client.pointer("/AudioSettings/AudioGroupPresets").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let mut given = 0;
    for preset in presets.iter_mut() {
        preset.direct_binaural = true;
        let Some(entry) = backend.iter().find(|e| e.get("GroupType").and_then(|v| v.as_i64()) == Some(preset.group as i64)) else {
            continue;
        };
        let float = |name: &str| entry.get(name).and_then(|v| v.as_f64()).map(|v| v as f32);
        // Absent from the JSON, a figure takes the class's default
        // (`GlobalAudioGroupPreset`).
        preset.overall_volume = float("OverallVolume").unwrap_or(0.0);
        preset.disable_binaural_by_dist = entry.get("DisabledBinauralByDistance").and_then(|v| v.as_bool()).unwrap_or(false);
        preset.height_diff_to_allow_binaural = float("HeightToAllowBinaural").unwrap_or(10.0);
        preset.angle_to_allow_binaural = float("AngleToAllowBinaural").unwrap_or(30.0);
        preset.enable_binaural_dist = float("DistanceToAllowBinaural").unwrap_or(60.0);
        given += 1;
    }
    given
}

// ==== The layout ====

/// Reads one preset, failing unless the layout ends on the object's last
/// byte.
fn parse(bytes: &[u8], big_endian: bool, resource_path: String) -> Result<AudioGroupPreset> {
    let mut r = Reader { data: bytes, pos: 0, big_endian };
    r.skip(12)?; // owning game object (none for an asset)
    r.skip(4)?; // enabled flag, padded
    r.skip(12)?; // script
    let name = string(&mut r)?;
    let group = r.i32()?;
    let overall_volume = r.f32()?;
    let spatial_blend = r.f32()?;
    let steam_spatialize = flag(&mut r)?;
    let direct_binaural = flag(&mut r)?;
    let disable_binaural_by_dist = flag(&mut r)?;
    let enable_binaural_dist = r.f32()?;
    let height_diff_to_allow_binaural = r.f32()?;
    let angle_to_allow_binaural = r.f32()?;
    let hrtf_intensity = r.f32()?;
    let directivity_pattern = r.i32()?;
    let directivity_intensity = r.f32()?;
    let occlusion_source_radius = r.f32()?;
    let (sound_rolloff, sound_rolloff_weighted) = curve(&mut r).context("SoundRolloff")?;
    let (spread_curve, _) = curve(&mut r).context("SpreadCurve")?;
    let priority = r.i32()?;
    let pre_cached_sources = r.i32()?;
    let sources_limit = r.i32()?;
    let default_max_distance = r.f32()?;
    let acoustic = AcousticSettings { mono: reverb(&mut r)?, stereo: reverb(&mut r)?, prewarm: flag(&mut r)? };
    let occlusion = OcclusionSettings {
        enabled: flag(&mut r)?,
        intensity: r.f32()?,
        max_quality_factor: r.f32()?,
        use_quality_compression: flag(&mut r)?,
        indoor_to_outdoor: r.f32()?,
        outdoor_to_indoor: r.f32()?,
        rolloff_scale: r.f32()?,
        obstruction: eq_preset(&mut r).context("obstructionEQPreset")?,
        propagation: eq_preset(&mut r).context("propagationEQPreset")?,
        stairs_height_curve: curve(&mut r).context("stairsHeightCurve")?.0,
    };
    ensure!(r.pos == bytes.len(), "the layout ends at byte {} of {}", r.pos, bytes.len());
    ensure!(group_name(group) != "", "group {group} is not one of the enum's");
    Ok(AudioGroupPreset {
        name,
        resource_path,
        group,
        overall_volume,
        spatial_blend,
        steam_spatialize,
        direct_binaural,
        disable_binaural_by_dist,
        enable_binaural_dist,
        height_diff_to_allow_binaural,
        angle_to_allow_binaural,
        hrtf_intensity,
        directivity_pattern,
        directivity_intensity,
        occlusion_source_radius,
        sound_rolloff,
        sound_rolloff_weighted,
        spread_curve,
        priority,
        pre_cached_sources,
        sources_limit,
        default_max_distance,
        acoustic,
        occlusion,
    })
}

fn reverb(r: &mut Reader) -> Result<Reverb> {
    Ok(Reverb { enabled: flag(r)?, reverb_send_db: r.f32()?, early_reflections_send_db: r.f32()?, reverb_reach: r.f32()? })
}

fn eq_preset(r: &mut Reader) -> Result<EqPreset> {
    Ok(EqPreset {
        volume_curve: curve(r).context("volumeCurve")?.0,
        environment_volume_thresholds: floats(r)?,
        height_volume_curve: curve(r).context("heightVolumeCurve")?.0,
        high_pass: filter(r).context("hpfSettings")?,
        low_pass: filter(r).context("lpfSettings")?,
        distance_coefficient: r.f32()?,
        rotation_coefficient: r.f32()?,
    })
}

fn filter(r: &mut Reader) -> Result<FilterSettings> {
    Ok(FilterSettings {
        frequency_curve: curve(r).context("frequencyCurve")?.0,
        resonance_curve: curve(r).context("resonanceCurve")?.0,
        distance_curve: curve(r).context("distanceCurve")?.0,
        height_curve: curve(r).context("heightCurve")?.0,
        position_thresholds: floats(r)?,
        environment_thresholds: floats(r)?,
    })
}

fn floats<const N: usize>(r: &mut Reader) -> Result<[f32; N]> {
    let mut out = [0.0; N];
    for v in &mut out {
        *v = r.f32()?;
    }
    Ok(out)
}

/// A flag: one byte, then padding to four.
fn flag(r: &mut Reader) -> Result<bool> {
    let v = r.u8()?;
    r.align(4);
    ensure!(v <= 1, "a flag of {v}");
    Ok(v == 1)
}

fn string(r: &mut Reader) -> Result<String> {
    let len = r.u32()? as usize;
    ensure!(len < 4096, "a name of {len} bytes");
    let s = String::from_utf8(r.bytes(len)?.to_vec()).context("a name that is not text")?;
    r.align(4);
    Ok(s)
}

/// An `AnimationCurve`: its keys (time, value, the two slopes, the weighted
/// mode and the two weights), then the wraps before and after and the
/// rotation order. Says too whether any key is weighted.
fn curve(r: &mut Reader) -> Result<(Curve, bool)> {
    let count = r.u32()? as usize;
    if count > 4096 {
        bail!("{count} keys is not a plausible curve");
    }
    let mut keys = Vec::with_capacity(count);
    let mut weighted = false;
    for _ in 0..count {
        let key = Key { time: r.f32()?, value: r.f32()?, in_slope: r.f32()?, out_slope: r.f32()? };
        let mode = r.i32()?;
        ensure!((0..=3).contains(&mode), "a key's weighted mode of {mode}");
        weighted |= mode != 0;
        r.skip(8)?; // the two weights
        keys.push(key);
    }
    r.skip(12)?; // wrap modes before and after, rotation order
    Ok((Curve { keys }, weighted))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every preset of the install, read to its last byte (`parse`), with
    /// its rolloff: `cargo test -p trp-data audio_preset -- --nocapture`
    /// prints them. A curve of more than eight keys is shown by its first
    /// three and its last two, and every one by its gain at shares of the
    /// maximum distance.
    #[test]
    fn the_presets_are_read_exactly() {
        let Ok(install) = Install::locate() else { return };
        let presets = audio_group_presets(&install).expect("the audio group presets");
        for p in &presets {
            println!(
                "{} ({}): group {} {}, volume {}, blend {}, max distance {}, priority {}, rolloff scale {}, weighted {}",
                p.name,
                p.resource_path,
                p.group,
                group_name(p.group),
                p.overall_volume,
                p.spatial_blend,
                p.default_max_distance,
                p.priority,
                p.occlusion.rolloff_scale,
                p.sound_rolloff_weighted
            );
            let keys = &p.sound_rolloff.keys;
            let shown: Vec<usize> = if keys.len() <= 8 { (0..keys.len()).collect() } else { vec![0, 1, 2, keys.len() - 2, keys.len() - 1] };
            println!("    rolloff: {} keys", keys.len());
            for i in shown {
                let k = keys[i];
                println!("    rolloff [{i}] t {:.6} v {:.6} in {:.6} out {:.6}", k.time, k.value, k.in_slope, k.out_slope);
            }
            let gains: Vec<String> = [0.01, 0.05, 0.1, 0.25, 0.5, 0.75, 1.0].iter().map(|s| format!("{s}: {:.4}", p.sound_rolloff.evaluate(*s))).collect();
            println!("    gain by share: {}", gains.join(", "));
            for k in &p.spread_curve.keys {
                println!("    spread t {:.6} v {:.6}", k.time, k.value);
            }
        }
        // And the backend's figures over them (`settings.json`).
        let settings = std::fs::read_to_string(install.database_dir().join("settings.json")).ok();
        if let Some(client) = settings.and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok()).and_then(|v| v.get("config").cloned()) {
            let mut given = presets.clone();
            let n = apply_client_settings(&mut given, &client);
            println!("the backend's figures for {n} presets:");
            for (a, b) in presets.iter().zip(&given) {
                println!(
                    "    {}: volume {} -> {}, binaural off by distance {} -> {} (beyond {} m, over {} m high, past {} degrees)",
                    a.name,
                    a.overall_volume,
                    b.overall_volume,
                    a.disable_binaural_by_dist,
                    b.disable_binaural_by_dist,
                    b.enable_binaural_dist,
                    b.height_diff_to_allow_binaural,
                    b.angle_to_allow_binaural
                );
            }
        }
        // The seventeen `BetterAudio` loads by name.
        for wanted in [
            "Character",
            "Collisions",
            "Distant",
            "Environment",
            "Gunshots",
            "Grenades",
            "Impacts",
            "Speech",
            "Weaponary",
            "NonspatialBypass",
            "Nonspatial",
            "Voip",
            "Windows",
            "Lamp",
            "InteractiveObjects",
            "PropagatedExclusive",
            "LightOcclusion",
        ] {
            let path = format!("{RESOURCE_DIR}{}", wanted.to_lowercase());
            assert!(presets.iter().any(|p| p.resource_path == path), "no preset at {path}");
        }
        // Every rolloff runs from nought to one, a share of the maximum
        // distance (`Character`'s last key lies at 1.0015), and none is
        // weighted.
        for p in &presets {
            let (Some(first), Some(last)) = (p.sound_rolloff.keys.first(), p.sound_rolloff.keys.last()) else {
                panic!("{}: no rolloff", p.name)
            };
            assert_eq!(first.time, 0.0, "{}", p.name);
            assert!((0.99..=1.01).contains(&last.time), "{}: the rolloff's last key at {}", p.name, last.time);
            assert!(!p.sound_rolloff_weighted, "{}: a weighted rolloff", p.name);
        }
        let character = presets.iter().find(|p| p.name == "Character").expect("Character");
        assert_eq!((character.group, character.default_max_distance, character.spatial_blend), (3, 70.0, 1.0));
    }
}
