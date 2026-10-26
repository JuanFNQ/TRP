//! The game's sounds as the install keeps them (`docs/spec/sound.md`): an
//! audio clip (`AudioClip`) is a sound bank of FMOD's (FSB5) holding one
//! sample, kept beside its file (`m_Resource`), its samples 16-bit PCM or
//! IMA ADPCM (`m_CompressionFormat` 0 or 2; Vorbis, 1, is not read). Read
//! here: the interface's sounds (`Resources/Audio/UISoundsWrapper`, by the
//! game's `EUISoundType`) and the things' sounds (the bundle
//! `assets/content/audio/itemsounds/itemsounds.bundle`, `ItemSounds`: a
//! clip a kind of thing's sound group (`ItemSound`) and a way of handling
//! it, named `<group>_<way>`).

use std::collections::HashMap;

use anyhow::{bail, ensure, Context, Result};

use crate::install::Install;
use crate::unity::Value;

/// The engine's class number of an audio clip.
const AUDIO_CLIP: i32 = 83;

/// The bundle of the things' sounds.
pub const ITEM_SOUNDS: &str = "assets/content/audio/itemsounds/itemsounds.bundle";

/// A clip's samples, its channels interleaved.
#[derive(Debug, Clone)]
pub struct Clip {
    pub name: String,
    pub channels: u16,
    pub frequency: u32,
    pub samples: Vec<i16>,
}

impl Clip {
    /// The clip as a WAVE file's bytes (16-bit PCM), for an audio player.
    pub fn wav(&self) -> Vec<u8> {
        let data = self.samples.len() * 2;
        let mut out = Vec::with_capacity(44 + data);
        let block = self.channels as u32 * 2;
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data as u32).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&self.channels.to_le_bytes());
        out.extend_from_slice(&self.frequency.to_le_bytes());
        out.extend_from_slice(&(self.frequency * block).to_le_bytes());
        out.extend_from_slice(&(block as u16).to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(data as u32).to_le_bytes());
        for sample in &self.samples {
            out.extend_from_slice(&sample.to_le_bytes());
        }
        out
    }
}

/// A clip's samples from its sound bank (FSB5): the bank's header (its
/// version, its samples' headers' size, its names' size, its data's size
/// and its codec, then its hash), and the one sample's data after the
/// samples' headers and the names; 16-bit PCM as it is (FMOD's codec 2), or
/// IMA ADPCM (codec 7, FMOD's in the Xbox's blocks: 36 bytes a channel,
/// first each channel's predictor and step index, then 64 samples a
/// channel of four bits, the lower first, the channels taking four bytes
/// in turn). `length`: the clip's own count of frames, which the last block
/// is cut to.
pub fn fsb5_samples(bank: &[u8], channels: u16, length: usize) -> Result<Vec<i16>> {
    ensure!(bank.get(0..4) == Some(b"FSB5".as_slice()), "not a sound bank of FMOD's");
    let word = |at: usize| -> Result<u32> { Ok(u32::from_le_bytes(bank.get(at..at + 4).context("the sound bank is short")?.try_into()?)) };
    let version = word(4)?;
    let (headers, names, size, codec) = (word(12)? as usize, word(16)? as usize, word(20)? as usize, word(24)?);
    let start = if version == 0 { 0x40 } else { 0x3c } + headers + names;
    let data = bank.get(start..start + size).or_else(|| bank.get(start..)).context("the sound bank's data is short")?;
    match codec {
        2 => {
            let mut samples: Vec<i16> = data.chunks_exact(2).map(|pair| i16::from_le_bytes([pair[0], pair[1]])).collect();
            samples.truncate(length * channels as usize);
            Ok(samples)
        }
        7 => {
            let channels = channels.max(1) as usize;
            let mut out: Vec<i16> = Vec::with_capacity(length * channels);
            for block in data.chunks(36 * channels) {
                if block.len() < 36 * channels {
                    break;
                }
                let mut decoded = vec![[0i16; 64]; channels];
                for (c, samples) in decoded.iter_mut().enumerate() {
                    let header = &block[4 * c..4 * c + 4];
                    let mut predictor = i16::from_le_bytes([header[0], header[1]]) as i32;
                    let mut index = (header[2] as i32).clamp(0, 88);
                    for (n, sample) in samples.iter_mut().enumerate() {
                        // The eight samples of four bytes, a channel's every
                        // so many after the headers.
                        let chunk = n / 8;
                        let byte = block[4 * channels + (chunk * channels + c) * 4 + (n % 8) / 2];
                        let nibble = if n % 2 == 0 { byte & 0x0f } else { byte >> 4 } as i32;
                        let step = IMA_STEPS[index as usize];
                        let mut delta = step >> 3;
                        if nibble & 1 != 0 {
                            delta += step >> 2;
                        }
                        if nibble & 2 != 0 {
                            delta += step >> 1;
                        }
                        if nibble & 4 != 0 {
                            delta += step;
                        }
                        predictor = if nibble & 8 != 0 { predictor - delta } else { predictor + delta }.clamp(-32768, 32767);
                        index = (index + IMA_INDEX[nibble as usize]).clamp(0, 88);
                        *sample = predictor as i16;
                    }
                }
                for n in 0..64 {
                    for samples in &decoded {
                        out.push(samples[n]);
                    }
                }
            }
            out.truncate(length * channels);
            Ok(out)
        }
        other => bail!("the sound bank's codec {other} is not read (16-bit PCM and IMA ADPCM are)"),
    }
}

/// FMOD's codec number of Vorbis in a sound bank.
pub const FSB5_VORBIS: u32 = 15;

/// A Vorbis sample of a sound bank (FSB5, codec 15): its channels,
/// frequency and frames as its header and chunks give them, the CRC32 of
/// the setup header it was encoded with (its `VORBISDATA` chunk, 11: FMOD
/// leaves the three headers out of the bank and finds the setup header by
/// this in a table of its own), and its packets, each kept after its size
/// as 16 bits.
#[derive(Debug, Clone)]
pub struct VorbisSample {
    pub channels: u16,
    pub frequency: u32,
    pub frames: u64,
    pub setup_crc: u32,
    pub packets: Vec<Vec<u8>>,
}

/// Reads a bank's one Vorbis sample. The sample's header is 64 bits: the
/// lowest whether chunks follow, then the frequency's number (4 bits),
/// the channels' (2), the data's offset in 32 bytes (27) and the frames
/// (30); a chunk's header is 32 bits: the lowest whether another follows,
/// then its size (24) and its kind (7).
pub fn fsb5_vorbis(bank: &[u8]) -> Result<VorbisSample> {
    ensure!(bank.get(0..4) == Some(b"FSB5".as_slice()), "not a sound bank of FMOD's");
    let word = |at: usize| -> Result<u32> { Ok(u32::from_le_bytes(bank.get(at..at + 4).context("the sound bank is short")?.try_into()?)) };
    let version = word(4)?;
    let (headers, names, size, codec) = (word(12)? as usize, word(16)? as usize, word(20)? as usize, word(24)?);
    ensure!(codec == FSB5_VORBIS, "the sound bank's codec is {codec}, not Vorbis");
    let base = if version == 0 { 0x40 } else { 0x3c };
    let mode = u64::from_le_bytes(bank.get(base..base + 8).context("the sample's header is short")?.try_into()?);
    let mut frequency = [4000, 8000, 11000, 11025, 16000, 22050, 24000, 32000, 44100, 48000, 96000].get(((mode >> 1) & 0x0f) as usize).copied().unwrap_or(44100);
    let mut channels = [1u16, 2, 6, 8][((mode >> 5) & 0x03) as usize];
    let offset = (((mode >> 7) & 0x07ff_ffff) << 5) as usize;
    let frames = (mode >> 34) & 0x3fff_ffff;
    let (mut more, mut at, mut setup_crc) = (mode & 1 != 0, base + 8, None);
    while more {
        let chunk = word(at)?;
        let (length, kind) = (((chunk >> 1) & 0x00ff_ffff) as usize, chunk >> 25);
        more = chunk & 1 != 0;
        let body = at + 4;
        match kind {
            1 => channels = *bank.get(body).context("the channels' chunk is short")? as u16,
            2 => frequency = word(body)?,
            11 => setup_crc = Some(word(body)?),
            _ => {}
        }
        at = body + length;
    }
    let setup_crc = setup_crc.context("the Vorbis sample names no setup header")?;
    let start = base + headers + names + offset;
    let data = bank.get(start..(base + headers + names + size).min(bank.len())).context("the sound bank's data is short")?;
    let mut packets = Vec::new();
    let mut at = 0;
    while at + 2 <= data.len() {
        let length = u16::from_le_bytes([data[at], data[at + 1]]) as usize;
        if length == 0 || at + 2 + length > data.len() {
            break;
        }
        packets.push(data[at + 2..at + 2 + length].to_vec());
        at += 2 + length;
    }
    Ok(VorbisSample { channels, frequency, frames, setup_crc, packets })
}

/// FMOD's Vorbis setup headers, read from the game's engine as clips ask for
/// them (`vorbis_setup_headers`), the engine's bytes kept while they are.
pub struct VorbisSetups {
    path: std::path::PathBuf,
    engine: Option<Vec<u8>>,
    found: HashMap<u32, Vec<u8>>,
}

impl VorbisSetups {
    pub fn new(install: &Install) -> Self {
        Self { path: install.root.join("UnityPlayer.dll"), engine: None, found: HashMap::new() }
    }

    /// The setup header a sample names by its CRC32.
    pub fn get(&mut self, crc: u32) -> Result<&[u8]> {
        if !self.found.contains_key(&crc) {
            if self.engine.is_none() {
                self.engine = Some(std::fs::read(&self.path).with_context(|| self.path.display().to_string())?);
            }
            let engine = self.engine.as_deref().unwrap_or_default();
            let header = setup_header_in(engine, crc).with_context(|| format!("the game's engine has no Vorbis setup header {crc:08x}"))?;
            self.found.insert(crc, header);
        }
        Ok(self.found.get(&crc).map(Vec::as_slice).unwrap_or_default())
    }
}

/// The run of the engine's bytes from a setup packet's mark whose CRC32 is
/// `crc`, no longer than the way to the next mark.
fn setup_header_in(engine: &[u8], crc: u32) -> Option<Vec<u8>> {
    const MARK: &[u8] = b"\x05vorbis";
    let starts: Vec<usize> = engine.windows(MARK.len()).enumerate().filter(|(_, window)| *window == MARK).map(|(at, _)| at).collect();
    for (index, &start) in starts.iter().enumerate() {
        let end = starts.get(index + 1).copied().unwrap_or(engine.len()).min(start + (1 << 16));
        let mut hasher = crc32fast::Hasher::new();
        for at in start..end {
            hasher.update(&engine[at..at + 1]);
            if hasher.clone().finalize() == crc {
                return Some(engine[start..=at].to_vec());
            }
        }
    }
    None
}

/// FMOD's Vorbis setup headers as the game's engine keeps them: FMOD is
/// built into `UnityPlayer.dll`, and its table of the setup headers it
/// encodes with lies there, each header beginning with a setup packet's
/// mark (5, then `vorbis`). A bank names its header by the header's CRC32
/// (`VorbisSample::setup_crc`); each one asked for is found as the run of
/// bytes from a mark whose CRC32 is that, no longer than the way to the
/// next mark. Nothing of them is kept but what the install has.
pub fn vorbis_setup_headers(install: &Install, wanted: &[u32]) -> Result<HashMap<u32, Vec<u8>>> {
    let path = install.root.join("UnityPlayer.dll");
    let engine = std::fs::read(&path).with_context(|| path.display().to_string())?;
    const MARK: &[u8] = b"\x05vorbis";
    let starts: Vec<usize> = engine.windows(MARK.len()).enumerate().filter(|(_, window)| *window == MARK).map(|(at, _)| at).collect();
    let mut found = HashMap::new();
    for (index, &start) in starts.iter().enumerate() {
        let end = starts.get(index + 1).copied().unwrap_or(engine.len()).min(start + (1 << 16));
        let mut hasher = crc32fast::Hasher::new();
        for at in start..end {
            hasher.update(&engine[at..at + 1]);
            let crc = hasher.clone().finalize();
            if wanted.contains(&crc) && !found.contains_key(&crc) {
                found.insert(crc, engine[start..=at].to_vec());
            }
        }
    }
    Ok(found)
}

/// FMOD's block sizes for its Vorbis (as powers of two: 256 and 2048), which
/// the identification header it leaves out of a bank would have said.
/// `PROVISIONAL`: read from vgmstream's notes on the format, not from FMOD;
/// a clip decoded so comes out at its own count of frames.
const FSB_BLOCKSIZES: (u8, u8) = (8, 11);

/// A Vorbis sample decoded (`lewton`): the identification header FMOD leaves
/// out made again from the sample's channels and frequency with FMOD's block
/// sizes, the setup header the one it names (`vorbis_setup_headers`), and
/// its packets decoded in order, the channels interleaved, cut to its
/// frames.
pub fn decode_vorbis(sample: &VorbisSample, setup: &[u8]) -> Result<Vec<i16>> {
    let mut ident = Vec::with_capacity(30);
    ident.push(1u8);
    ident.extend_from_slice(b"vorbis");
    ident.extend_from_slice(&0u32.to_le_bytes());
    ident.push(sample.channels as u8);
    ident.extend_from_slice(&sample.frequency.to_le_bytes());
    for _ in 0..3 {
        ident.extend_from_slice(&0i32.to_le_bytes());
    }
    ident.push((FSB_BLOCKSIZES.1 << 4) | FSB_BLOCKSIZES.0);
    ident.push(1);
    let ident = lewton::header::read_header_ident(&ident).map_err(|e| anyhow::anyhow!("the identification header made again: {e:?}"))?;
    let setup = lewton::header::read_header_setup(setup, ident.audio_channels, (ident.blocksize_0, ident.blocksize_1)).map_err(|e| anyhow::anyhow!("the setup header: {e:?}"))?;
    let mut previous = lewton::audio::PreviousWindowRight::new();
    let channels = sample.channels.max(1) as usize;
    let mut out: Vec<i16> = Vec::with_capacity(sample.frames as usize * channels);
    for (index, packet) in sample.packets.iter().enumerate() {
        let decoded = lewton::audio::read_audio_packet(&ident, &setup, packet, &mut previous).map_err(|e| anyhow::anyhow!("packet {index}: {e:?}"))?;
        let frames = decoded.first().map_or(0, Vec::len);
        for frame in 0..frames {
            for channel in &decoded {
                out.push(channel[frame]);
            }
        }
    }
    out.truncate(sample.frames as usize * channels);
    Ok(out)
}

/// The things' sounds kept as Vorbis (`m_CompressionFormat` 1), by the
/// clip's name, each with its sample as its bank has it.
pub fn item_vorbis(install: &Install) -> Result<Vec<(String, VorbisSample)>> {
    let mut assets = crate::unity::Assets::new(install.clone())?;
    let files = assets.load(ITEM_SOUNDS)?;
    let mut out = Vec::new();
    for file in files {
        let ids: Vec<i64> = assets.file(file).meta.objects.iter().filter(|object| object.class_id == AUDIO_CLIP).map(|object| object.path_id).collect();
        for id in ids {
            let clip = assets.read(crate::unity::ObjRef { file, path_id: id })?;
            if clip.get("m_CompressionFormat").and_then(|v| v.as_i64()) != Some(1) {
                continue;
            }
            let name = clip.get("m_Name").and_then(|v| v.as_str()).unwrap_or_default().to_owned();
            let resource = clip.get("m_Resource").context("no resource")?;
            let source = resource.get("m_Source").and_then(|v| v.as_str()).unwrap_or_default().to_owned();
            let offset = resource.get("m_Offset").and_then(|v| v.as_usize()).unwrap_or(0);
            let size = resource.get("m_Size").and_then(|v| v.as_usize()).unwrap_or(0);
            let bank = assets.resource(&source, offset, size)?.to_vec();
            out.push((name.clone(), fsb5_vorbis(&bank).with_context(|| name)?));
        }
    }
    Ok(out)
}

const IMA_INDEX: [i32; 16] = [-1, -1, -1, -1, 2, 4, 6, 8, -1, -1, -1, -1, 2, 4, 6, 8];
const IMA_STEPS: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66, 73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449, 494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272, 2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493, 10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767,
];

/// A clip's fields as a type tree reads them, its bank given its bytes.
pub(crate) fn clip_of(clip: &Value, bank: &dyn Fn(&str, usize, usize) -> Result<Vec<u8>>, setups: &mut VorbisSetups) -> Result<Clip> {
    let name = clip.get("m_Name").and_then(|v| v.as_str()).unwrap_or_default().to_owned();
    let channels = clip.get("m_Channels").and_then(|v| v.as_i64()).unwrap_or(1).max(1) as u16;
    let frequency = clip.get("m_Frequency").and_then(|v| v.as_i64()).unwrap_or(44100).max(1) as u32;
    let length = clip.get("m_Length").and_then(|v| v.as_f32()).unwrap_or(0.0);
    let format = clip.get("m_CompressionFormat").and_then(|v| v.as_i64()).unwrap_or(-1);
    ensure!(format == 0 || format == 1 || format == 2, "{name}: kept as {format}, which is not read");
    // Vorbis: its headers made again from the bank and the engine.
    if format == 1 {
        let resource = clip.get("m_Resource").context("no resource")?;
        let source = resource.get("m_Source").and_then(|v| v.as_str()).unwrap_or_default();
        let offset = resource.get("m_Offset").and_then(|v| v.as_usize()).unwrap_or(0);
        let size = resource.get("m_Size").and_then(|v| v.as_usize()).unwrap_or(0);
        let bytes = bank(source, offset, size).with_context(|| name.clone())?;
        let sample = fsb5_vorbis(&bytes).with_context(|| name.clone())?;
        let setup = setups.get(sample.setup_crc).with_context(|| name.clone())?.to_vec();
        let samples = decode_vorbis(&sample, &setup).with_context(|| name.clone())?;
        return Ok(Clip { name, channels: sample.channels, frequency: sample.frequency, samples });
    }
    let resource = clip.get("m_Resource").context("no resource")?;
    let source = resource.get("m_Source").and_then(|v| v.as_str()).unwrap_or_default();
    let offset = resource.get("m_Offset").and_then(|v| v.as_usize()).unwrap_or(0);
    let size = resource.get("m_Size").and_then(|v| v.as_usize()).unwrap_or(0);
    let bytes = bank(source, offset, size).with_context(|| name.clone())?;
    let frames = (length * frequency as f32).round() as usize;
    let samples = fsb5_samples(&bytes, channels, frames).with_context(|| name.clone())?;
    Ok(Clip { name, channels, frequency, samples })
}

/// The interface's sounds: those of the kinds asked, by kind, and the two
/// lists a magazine's rounds are loaded and unloaded to (inventory spec,
/// "Loading and unloading a magazine").
pub struct UiSounds {
    pub by_kind: HashMap<i32, Clip>,
    /// `_loadSounds`, `_unloadSounds`, in their order.
    pub load: Vec<Clip>,
    pub unload: Vec<Clip>,
    /// A notice's sound (`GUISounds.PlayNotificationSound`: the Resource
    /// `Audio/Interface Sounds/notification_exp`), where it is had.
    pub notification: Option<Clip>,
}

/// The interface's sounds of the kinds asked (`EUISoundType`'s numbers), by
/// kind: the Resource `audio/uisoundswrapper` (`UISoundsWrapper`), whose
/// fields after a script's own begin with its list of a kind and a clip's
/// pointer each (`_UIAudioClips`), the clips read from `resources.assets`
/// by an audio clip's type tree borrowed from the things' bundle, their
/// banks from the file their resource names beside it. After that list
/// come two more of the same shape (the social network's, the game's end's)
/// and then the load's and the unload's, each a list of clips' pointers.
pub fn ui_sounds(install: &Install, kinds: &[i32]) -> Result<UiSounds> {
    let mut setups = VorbisSetups::new(install);
    let paths = crate::screen::resource_paths(install)?;
    let (_, file, id) = paths.iter().find(|(path, ..)| path == "audio/uisoundswrapper").context("no resource is at audio/uisoundswrapper")?;
    let (_, bytes) = crate::screen::object_bytes(install, file, *id)?;
    let int = |at: usize| -> Result<i64> { Ok(i32::from_le_bytes(bytes.get(at..at + 4).context("the sounds' list is short")?.try_into()?) as i64) };
    // The object, its switch, its script, its name (four-aligned).
    let mut at = 12 + 4 + 12;
    at += 4 + int(at)? as usize;
    at = (at + 3) & !3;
    let count = int(at)? as usize;
    at += 4;
    let tree = crate::screen::tree_of(install, ITEM_SOUNDS, AUDIO_CLIP)?;
    let mut pictures = crate::screen::ScenePictures::new(install)?;
    let folder = install.root.join("EscapeFromTarkov_Data");
    let bank = |source: &str, offset: usize, size: usize| -> Result<Vec<u8>> {
        use std::io::{Read, Seek, SeekFrom};
        let path = folder.join(source.rsplit('/').next().unwrap_or(source));
        let mut file = std::fs::File::open(&path).with_context(|| path.display().to_string())?;
        file.seek(SeekFrom::Start(offset as u64))?;
        let mut data = vec![0; size];
        file.read_exact(&mut data)?;
        Ok(data)
    };
    let mut out = HashMap::new();
    for n in 0..count {
        let entry = at + 16 * n;
        let kind = int(entry)? as i32;
        if !kinds.contains(&kind) || int(entry + 4)? != 0 {
            continue;
        }
        let clip_id = i64::from_le_bytes(bytes.get(entry + 8..entry + 16).context("the sounds' list is short")?.try_into()?);
        let value = pictures.read_by(file, clip_id, AUDIO_CLIP, &tree)?;
        match clip_of(&value, &bank, &mut setups) {
            Ok(clip) => {
                out.insert(kind, clip);
            }
            Err(e) => eprintln!("the interface's sound {kind} could not be read: {e:#}"),
        }
    }
    // Past the two lists of a kind and a clip each, the two of clips.
    let mut lists = at + 16 * count;
    for _ in 0..2 {
        lists += 4 + 16 * int(lists)? as usize;
    }
    let mut clips_of = |at: &mut usize| -> Result<Vec<Clip>> {
        let count = int(*at)? as usize;
        let mut clips = Vec::new();
        for n in 0..count {
            let entry = *at + 4 + 12 * n;
            if int(entry)? != 0 {
                continue;
            }
            let clip_id = i64::from_le_bytes(bytes.get(entry + 4..entry + 12).context("the sounds' list is short")?.try_into()?);
            let value = pictures.read_by(file, clip_id, AUDIO_CLIP, &tree)?;
            clips.push(clip_of(&value, &bank, &mut setups)?);
        }
        *at += 4 + 12 * count;
        Ok(clips)
    };
    let load = clips_of(&mut lists)?;
    let unload = clips_of(&mut lists)?;
    let notification = paths.iter().find(|(path, ..)| path == "audio/interface sounds/notification_exp").and_then(|(_, file, id)| {
        let value = pictures.read_by(file, *id, AUDIO_CLIP, &tree).ok()?;
        clip_of(&value, &bank, &mut setups).map_err(|e| eprintln!("the notices' sound could not be read: {e:#}")).ok()
    });
    Ok(UiSounds { by_kind: out, load, unload, notification })
}

/// The things' sounds (`ItemSounds`), every one, by its clip's name
/// (`<group>_<way>`: `weap_rifle_drop`; the ways `drop`, `pickup`, `use`,
/// `open`, `other`, `offline_use`, `EInventorySoundType`).
pub fn item_sounds(install: &Install) -> Result<HashMap<String, Clip>> {
    let mut setups = VorbisSetups::new(install);
    let mut assets = crate::unity::Assets::new(install.clone())?;
    let files = assets.load(ITEM_SOUNDS)?;
    let mut out = HashMap::new();
    // Those that could not be read, said once together.
    let mut unread = Vec::new();
    for file in files {
        let ids: Vec<i64> = assets.file(file).meta.objects.iter().filter(|object| object.class_id == AUDIO_CLIP).map(|object| object.path_id).collect();
        for id in ids {
            let value = assets.read(crate::unity::ObjRef { file, path_id: id })?;
            let bank = |source: &str, offset: usize, size: usize| -> Result<Vec<u8>> { Ok(assets.resource(source, offset, size)?.to_vec()) };
            match clip_of(&value, &bank, &mut setups) {
                Ok(clip) => {
                    out.insert(clip.name.clone(), clip);
                }
                Err(e) => unread.push(format!("{e:#}")),
            }
        }
    }
    if !unread.is_empty() {
        eprintln!("{} of the things' sounds are not read and are silent: {}", unread.len(), unread.join("; "));
    }
    Ok(out)
}

/// A sound a hands rig plays for a mark of its animator
/// (`BaseSoundPlayer.SoundElement`): the mark's name, how far off it is
/// heard (its rolloff, in metres), its volume, and its clips, one of which
/// plays at random each time.
#[derive(Debug, Clone)]
pub struct SoundElement {
    pub event: String,
    pub rolloff: i32,
    pub volume: f32,
    pub clips: Vec<Clip>,
}

/// A hands rig's sound player (`BaseSoundPlayer` or a kind of it, on a
/// weapon's or an item's own prefab): its script's class and its two lists,
/// `MainSounds` (by the name of an event of the weapon's own) and
/// `AdditionalSounds` (by a mark's string: the `Sound` and `SoundAtPoint`
/// marks).
#[derive(Debug, Clone, Default)]
pub struct HandsSounds {
    pub player: String,
    pub main: Vec<SoundElement>,
    pub additional: Vec<SoundElement>,
    /// A weapon's shots, where its player is a `WeaponSoundPlayer` with
    /// banks of them.
    pub shots: Option<ShotSounds>,
}

/// A weapon's sound player's banks of shots (`WeaponSoundPlayer`), in this
/// order: `Body` (a shot, or for an automatic weapon its fire as a loop),
/// `Tail` (what an automatic weapon's fire ends with), `Doublet` (two
/// chambers fired at once), and the three with a silencer.
pub const SHOT_BANKS: [&str; 6] = ["Body", "Tail", "Doublet", "BodySilenced", "TailSilenced", "DoubletSilenced"];

/// A bank of a weapon's shots (`SoundBank`): its figures, and the clips of
/// the outdoor environment (`EnvironmentType.Outdoor`, the first of its
/// `Environments`; the app's range has no indoors) in its three lists by
/// distance (`DistanceVarity`), each clip by its place in
/// `ShotSounds::clips`.
#[derive(Debug, Clone)]
pub struct ShotBank {
    pub name: String,
    pub base_volume: f32,
    pub rolloff: f32,
    /// `ClipLength`: its `CustomLength` if that is not 0, else the length
    /// kept with it (`_clipLiength`). An automatic weapon's loop is 16 beats
    /// of it.
    pub clip_length: f32,
    /// Its `DistanceBlendOptions.Values`, if it has them.
    pub blend: Option<Vec<f32>>,
    pub lists: [Vec<usize>; 3],
}

/// A weapon's shots as its sound player keeps them.
#[derive(Debug, Clone, Default)]
pub struct ShotSounds {
    /// `Non_auto`: each shot plays whole, not as a loop.
    pub non_auto: bool,
    /// By `SHOT_BANKS`' order; a bank the player has not is `None`.
    pub banks: Vec<Option<ShotBank>>,
    /// Every clip the banks name, read once (one not read is `None`), with
    /// its name.
    pub clips: Vec<(String, Option<Clip>)>,
}

impl ShotSounds {
    /// A bank by its name in `SHOT_BANKS`.
    pub fn bank(&self, name: &str) -> Option<&ShotBank> {
        SHOT_BANKS.iter().position(|n| *n == name).and_then(|at| self.banks.get(at)).and_then(Option::as_ref)
    }
}

/// A sound player's banks of shots, with their clips (each read once).
fn shot_sounds(assets: &crate::unity::Assets, behaviour: crate::unity::ObjRef, fields: &Value, setups: &mut VorbisSetups) -> Result<Option<ShotSounds>> {
    use crate::unity::ObjRef;
    let mut out = ShotSounds { non_auto: fields.get("Non_auto").and_then(Value::as_bool).unwrap_or(false), ..Default::default() };
    let mut clip_places: HashMap<ObjRef, usize> = HashMap::new();
    let mut bank_read: HashMap<ObjRef, ShotBank> = HashMap::new();
    let mut unread = Vec::new();
    for name in SHOT_BANKS {
        let Some(at) = fields.get(name).and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(behaviour.file, p)) else {
            out.banks.push(None);
            continue;
        };
        if let Some(bank) = bank_read.get(&at) {
            out.banks.push(Some(bank.clone()));
            continue;
        }
        let bank = assets.read(at)?;
        let number = |name: &str, or: f32| bank.get(name).and_then(Value::as_f32).unwrap_or(or);
        let blend = bank
            .get("BlendOptions")
            .and_then(Value::as_pptr)
            .and_then(|p| assets.resolve_loaded(at.file, p))
            .and_then(|options| assets.read(options).ok())
            .and_then(|options| options.get("Values").and_then(Value::as_floats).map(<[f32]>::to_vec));
        let mut lists: [Vec<usize>; 3] = Default::default();
        let first = bank.get("Environments").and_then(Value::as_array).and_then(|all| all.first());
        let varieties = first.and_then(|e| e.get("Clips")).and_then(Value::as_array).unwrap_or(&[]);
        for (list, variety) in lists.iter_mut().zip(varieties) {
            for pointer in variety.get("Clips").and_then(Value::as_array).unwrap_or(&[]) {
                let Some(clip) = pointer.as_pptr().and_then(|p| assets.resolve_loaded(at.file, p)) else { continue };
                let place = match clip_places.get(&clip) {
                    Some(&place) => place,
                    None => {
                        let value = assets.read(clip)?;
                        let name = value.get("m_Name").and_then(Value::as_str).unwrap_or_default().to_owned();
                        let resource = |source: &str, offset: usize, size: usize| -> Result<Vec<u8>> { Ok(assets.resource(source, offset, size)?.to_vec()) };
                        let read = match clip_of(&value, &resource, setups) {
                            Ok(clip) => Some(clip),
                            Err(e) => {
                                unread.push(format!("{e:#}"));
                                None
                            }
                        };
                        out.clips.push((name, read));
                        clip_places.insert(clip, out.clips.len() - 1);
                        out.clips.len() - 1
                    }
                };
                list.push(place);
            }
        }
        let custom = number("CustomLength", 0.0);
        let read = ShotBank {
            name: bank.get("m_Name").and_then(Value::as_str).unwrap_or_default().to_owned(),
            base_volume: number("BaseVolume", 1.0),
            rolloff: number("Rolloff", 100.0),
            clip_length: if custom != 0.0 { custom } else { number("_clipLiength", 0.0) },
            blend,
            lists,
        };
        bank_read.insert(at, read.clone());
        out.banks.push(Some(read));
    }
    if !unread.is_empty() {
        eprintln!("{} of the shots' sounds are not read and are silent: {}", unread.len(), unread.join("; "));
    }
    Ok(out.banks.iter().any(Option::is_some).then_some(out))
}

impl HandsSounds {
    /// The sound for a mark's string (`SoundEventHandler`): the element of
    /// `AdditionalSounds` named so, or so with `Snd` before it.
    pub fn for_mark(&self, name: &str) -> Option<&SoundElement> {
        self.additional.iter().find(|element| element.event == name || element.event.strip_prefix("Snd") == Some(name))
    }
}

/// The script on an object whose class's name ends in `SoundPlayer`, with
/// its class.
fn sound_player_on(assets: &crate::unity::Assets, object: crate::unity::ObjRef) -> Option<(String, crate::unity::ObjRef)> {
    let value = assets.read(object).ok()?;
    let components: Vec<crate::unity::ObjRef> =
        value.get("m_Component").and_then(Value::as_array).unwrap_or(&[]).iter().filter_map(|c| c.get("component")?.as_pptr()).filter_map(|p| assets.resolve_loaded(object.file, p)).collect();
    components.into_iter().find_map(|component| crate::unity::extract::script_class(assets, component).filter(|class| class.ends_with("SoundPlayer")).map(|class| (class, component)))
}

/// A rig's sound player: on the prefab's own object (beside its
/// `WeaponPrefab` or `UsableHandsPrefab`, above the animated rig), else on
/// one of the rig's objects; its lists read with their clips (each clip
/// read once), and its banks of shots if `shots` and it has them.
pub fn hands_sounds(assets: &crate::unity::Assets, weapon: &crate::weapon::Weapon, setups: &mut VorbisSetups, shots: bool) -> Result<Option<HandsSounds>> {
    let root = weapon.prefab.get("m_GameObject").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(weapon.prefab_file, p));
    let nodes = weapon.hierarchy.nodes.iter().filter(|node| node.scripts.iter().any(|class| class.ends_with("SoundPlayer"))).filter_map(|node| {
        let transform = assets.read(node.transform).ok()?;
        transform.get("m_GameObject").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(node.transform.file, p))
    });
    let found = root.into_iter().chain(nodes).find_map(|object| sound_player_on(assets, object));
    {
        let Some((player, behaviour)) = found else { return Ok(None) };
        let fields = assets.read(behaviour)?;
        let mut read_clips: HashMap<crate::unity::ObjRef, Clip> = HashMap::new();
        let mut list = |name: &str| -> Result<Vec<SoundElement>> {
            let mut out = Vec::new();
            for element in fields.get(name).and_then(Value::as_array).unwrap_or(&[]) {
                let mut clips = Vec::new();
                for clip in element.get("SoundClips").and_then(Value::as_array).unwrap_or(&[]) {
                    let Some(at) = clip.as_pptr().and_then(|p| assets.resolve_loaded(behaviour.file, p)) else { continue };
                    if !read_clips.contains_key(&at) {
                        let value = assets.read(at)?;
                        let bank = |source: &str, offset: usize, size: usize| -> Result<Vec<u8>> { Ok(assets.resource(source, offset, size)?.to_vec()) };
                        match clip_of(&value, &bank, setups) {
                            Ok(read) => {
                                read_clips.insert(at, read);
                            }
                            Err(e) => eprintln!("a hands sound could not be read: {e:#}"),
                        }
                    }
                    if let Some(read) = read_clips.get(&at) {
                        clips.push(read.clone());
                    }
                }
                out.push(SoundElement {
                    event: element.get("EventName").and_then(Value::as_str).unwrap_or_default().to_owned(),
                    rolloff: element.get("RollOff").and_then(Value::as_i64).unwrap_or(30) as i32,
                    volume: element.get("Volume").and_then(Value::as_f32).unwrap_or(1.0),
                    clips,
                });
            }
            Ok(out)
        };
        let main = list("MainSounds")?;
        let additional = list("AdditionalSounds")?;
        let shots = if shots && player == "WeaponSoundPlayer" { shot_sounds(assets, behaviour, &fields, setups)? } else { None };
        Ok(Some(HandsSounds { player, main, additional, shots }))
    }
}

/// The bundle of the game's audio mixer (`MasterMixer`, Unity's
/// `AudioMixerController` as built: its groups, effects and snapshots
/// compiled into one `AudioMixerConstant`).
pub const MASTER_MIXER: &str = "assets/content/audio/mastermixer.bundle";

/// A group of the game's mixer: its name, its parent's place, its volume
/// in the start snapshot (dB), whether a script may set that volume (an
/// exposed parameter), and its effects.
#[derive(Debug, Clone)]
pub struct MixerGroup {
    pub name: String,
    pub parent: Option<usize>,
    pub volume_db: f32,
    pub exposed: bool,
    pub effects: Vec<MixerEffect>,
    /// Where its volume is among a snapshot's values.
    pub volume_index: Option<usize>,
}

/// An effect of a group: its kind as the build numbers it (negative for the
/// mixer's own: -2 the group's attenuation, -3 a send, -4 a return; else
/// by my reading FMOD's own numbering of its effects, 16 its compressor,
/// 17 its reverb, 3 and 5 its low and high pass, 10 its equaliser) and its
/// parameters' values in the start snapshot.
#[derive(Debug, Clone)]
pub struct MixerEffect {
    pub kind: i64,
    pub parameters: Vec<f32>,
    /// Where each parameter is among a snapshot's values.
    pub indices: Vec<usize>,
    /// For a send (-3), the group it sends to (`sendTargetEffectIndex`'s
    /// effect's group), where it names one.
    pub target: Option<usize>,
}

/// What an exposed parameter of the mixer sets: a group's volume, or a
/// parameter of an effect of a group (a send's level, a filter's
/// frequency).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Exposed {
    Volume { group: usize },
    Effect { group: usize, effect: usize, parameter: usize },
}

/// The bundle of the weapons' other sounds (`WeaponSounds`: the shells',
/// the grenades', the mount's, and the aiming's by the weapon's kind).
pub const WEAPON_SOUNDS: &str = "assets/content/audio/prefabs/shells/weaponsounds.bundle";

/// A weapon's kind of sound (`ESoundWeaponType`, `AudioWeaponUtils.
/// GetWeaponType`) by its template's classes: a grenade launcher
/// `Launcher`; an assault rifle or carbine, a marksman or sniper rifle
/// `Riffle` (the game's spelling); `Shotgun`; `MachineGun`; a pistol or a
/// revolver `Pistol`; `SMG`; any other weapon `SpecialWeapon`.
pub fn sound_weapon_kind(db: &crate::Database, template: &crate::database::ItemTemplate) -> &'static str {
    let is = |class: &str| db.is_a(template, class);
    if is("GrenadeLauncher") {
        "Launcher"
    } else if is("AssaultRifle") || is("AssaultCarbine") || is("MarksmanRifle") || is("SniperRifle") {
        "Riffle"
    } else if is("Shotgun") {
        "Shotgun"
    } else if is("MachineGun") {
        "MachineGun"
    } else if is("Pistol") || is("Revolver") {
        "Pistol"
    } else if is("Smg") {
        "SMG"
    } else {
        "SpecialWeapon"
    }
}

/// A bank of the aiming's sounds (`SoundBankWithSettings`): its clips,
/// volume and rolloff.
#[derive(Debug, Clone)]
pub struct AimingBank {
    pub name: String,
    pub volume: f32,
    pub rolloff: f32,
    pub clips: Vec<Clip>,
}

/// The aiming's sounds (`WeaponSounds.AimingSounds`, a
/// `WeaponAimingSoundsSO` in a bundle `WEAPON_SOUNDS` depends on): each
/// kind of weapon's bank by the kind's name (`_keys`, `_values`), kinds
/// that share a bank sharing its clips' reading.
pub fn aiming_sounds(install: &Install) -> Result<Vec<(String, std::sync::Arc<AimingBank>)>> {
    use crate::unity::{class, extract};
    let mut setups = VorbisSetups::new(install);
    let mut assets = crate::unity::Assets::new(install.clone())?;
    let files = assets.load(WEAPON_SOUNDS)?;
    let sounds = files
        .iter()
        .flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR).collect::<Vec<_>>())
        .find(|&obj| extract::script_class(&assets, obj).as_deref() == Some("WeaponSounds"))
        .context("no WeaponSounds in its bundle")?;
    let value = assets.read(sounds)?;
    let so = value.get("AimingSounds").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(sounds.file, p)).context("the aiming's sounds lead nowhere loaded")?;
    let so_value = assets.read(so)?;
    let table = so_value.get("_aimingSounds").context("no _aimingSounds")?;
    let keys = table.get("_keys").and_then(Value::as_array).unwrap_or(&[]);
    let values = table.get("_values").and_then(Value::as_array).unwrap_or(&[]);
    let mut read: HashMap<crate::unity::ObjRef, std::sync::Arc<AimingBank>> = HashMap::new();
    let mut out = Vec::new();
    for (key, pointer) in keys.iter().zip(values) {
        let (Some(kind), Some(at)) = (key.as_str(), pointer.as_pptr().and_then(|p| assets.resolve_loaded(so.file, p))) else { continue };
        if let Some(bank) = read.get(&at) {
            out.push((kind.to_owned(), bank.clone()));
            continue;
        }
        let bank = assets.read(at)?;
        let mut clips = Vec::new();
        for pointer in bank.get("Clips").and_then(Value::as_array).unwrap_or(&[]) {
            let Some(clip) = pointer.as_pptr().and_then(|p| assets.resolve_loaded(at.file, p)) else { continue };
            let value = assets.read(clip)?;
            let resource = |source: &str, offset: usize, size: usize| -> Result<Vec<u8>> { Ok(assets.resource(source, offset, size)?.to_vec()) };
            match clip_of(&value, &resource, &mut setups) {
                Ok(clip) => clips.push(clip),
                Err(e) => eprintln!("an aiming sound could not be read: {e:#}"),
            }
        }
        let made = std::sync::Arc::new(AimingBank {
            name: bank.get("m_Name").and_then(Value::as_str).unwrap_or_default().to_owned(),
            volume: bank.get("BaseVolume").and_then(Value::as_f32).unwrap_or(1.0),
            rolloff: bank.get("Rolloff").and_then(Value::as_f32).unwrap_or(100.0),
            clips,
        });
        read.insert(at, made.clone());
        out.push((kind.to_owned(), made));
    }
    Ok(out)
}

/// The game's mixer as it starts (`m_StartSnapshot`).
#[derive(Debug, Clone, Default)]
pub struct Mixer {
    /// The exposed parameters (`exposedParameterNames`, each a CRC32 of
    /// the name scripts set it by, and `exposedParameterIndices`, where its
    /// value is).
    pub exposed: Vec<(u32, usize)>,
    pub start_snapshot: String,
    pub groups: Vec<MixerGroup>,
}

/// The effect kind read as FMOD's compressor (`FMOD_DSP_TYPE_COMPRESSOR`):
/// its parameters a threshold (dB), an attack and a release (ms) and a
/// make-up gain (dB).
pub const COMPRESSOR: i64 = 16;

impl Mixer {
    pub fn group(&self, name: &str) -> Option<usize> {
        self.groups.iter().position(|group| group.name == name)
    }

    /// What the exposed parameter of that name sets (`AudioMixer.SetFloat`'s
    /// name, found by its CRC32), where the mixer has it.
    pub fn exposed(&self, name: &str) -> Option<Exposed> {
        let hash = crc32fast::hash(name.as_bytes());
        let index = self.exposed.iter().find(|(named, _)| *named == hash)?.1;
        for (g, group) in self.groups.iter().enumerate() {
            if group.volume_index == Some(index) {
                return Some(Exposed::Volume { group: g });
            }
            for (e, effect) in group.effects.iter().enumerate() {
                if let Some(parameter) = effect.indices.iter().position(|&i| i == index) {
                    return Some(Exposed::Effect { group: g, effect: e, parameter });
                }
            }
        }
        None
    }

    /// A group's volume with every group above it, in dB, as the start
    /// snapshot has them.
    pub fn path_db(&self, group: usize) -> f32 {
        let mut at = Some(group);
        let mut db = 0.0;
        while let Some(index) = at {
            db += self.groups[index].volume_db;
            at = self.groups[index].parent;
        }
        db
    }

    /// The master group's compressor, if it has one: threshold (dB),
    /// attack and release (s), make-up gain (dB).
    pub fn master_compressor(&self) -> Option<[f32; 4]> {
        let master = self.groups.iter().find(|group| group.parent.is_none() && group.name == "Master")?;
        let effect = master.effects.iter().find(|effect| effect.kind == COMPRESSOR && effect.parameters.len() >= 4)?;
        let p = &effect.parameters;
        Some([p[0], p[1] / 1000.0, p[2] / 1000.0, p[3]])
    }
}

/// The game's mixer (`MASTER_MIXER`): each group named by its GUID among
/// the bundle's group objects, its volume and its effects' parameters by
/// their places in the start snapshot's values.
pub fn master_mixer(install: &Install) -> Result<Mixer> {
    use crate::unity::ObjRef;
    let mut assets = crate::unity::Assets::new(install.clone())?;
    let file = *assets.load(MASTER_MIXER)?.first().context("the mixer's bundle is empty")?;
    let guid = |v: &Value| -> Option<[i64; 4]> { Some([v.get("data[0]")?.as_i64()?, v.get("data[1]")?.as_i64()?, v.get("data[2]")?.as_i64()?, v.get("data[3]")?.as_i64()?]) };
    let mut names: HashMap<[i64; 4], String> = HashMap::new();
    let mut mixer = None;
    let objects: Vec<(i64, i32)> = assets.file(file).meta.objects.iter().map(|o| (o.path_id, o.class_id)).collect();
    for (path_id, class_id) in objects {
        let object = ObjRef { file, path_id };
        match class_id {
            243 | 245 => {
                let value = assets.read(object)?;
                let id = value.get(if class_id == 243 { "m_GroupID" } else { "m_SnapshotID" }).and_then(guid);
                if let Some(id) = id {
                    names.insert(id, value.get("m_Name").and_then(Value::as_str).unwrap_or_default().to_owned());
                }
            }
            241 => mixer = Some((object, assets.read(object)?)),
            _ => {}
        }
    }
    let (object, mixer) = mixer.context("no mixer in its bundle")?;
    let constant = mixer.get("m_MixerConstant").context("the mixer has no constant")?;
    let list = |name: &str| constant.get(name).and_then(Value::as_array).unwrap_or(&[]);
    // The start snapshot's place among the snapshots, by its GUID.
    let start = mixer
        .get("m_StartSnapshot")
        .and_then(Value::as_pptr)
        .and_then(|p| assets.resolve_loaded(object.file, p))
        .and_then(|snapshot| assets.read(snapshot).ok())
        .and_then(|snapshot| snapshot.get("m_SnapshotID").and_then(guid));
    let snapshot_ids: Vec<Option<[i64; 4]>> = list("snapshotGUIDs").iter().map(guid).collect();
    let at = snapshot_ids.iter().position(|id| id.is_some() && *id == start).context("the start snapshot is not among the snapshots")?;
    let values = list("snapshots").get(at).and_then(|s| s.get("values")).and_then(Value::as_floats).context("the start snapshot has no values")?;
    let exposed: Vec<u32> = match constant.get("exposedParameterIndices") {
        Some(Value::UInts(indices)) => indices.clone(),
        _ => Vec::new(),
    };
    let int = |v: &Value, name: &str| v.get(name).and_then(Value::as_i64).unwrap_or(-1);
    let group_ids: Vec<Option<[i64; 4]>> = list("groupGUIDs").iter().map(guid).collect();
    let mut groups: Vec<MixerGroup> = list("groups")
        .iter()
        .enumerate()
        .map(|(n, group)| {
            let volume = int(group, "volumeIndex");
            MixerGroup {
                name: group_ids.get(n).copied().flatten().and_then(|id| names.get(&id).cloned()).unwrap_or_default(),
                parent: usize::try_from(int(group, "parentConstantIndex")).ok(),
                volume_db: usize::try_from(volume).ok().and_then(|i| values.get(i)).copied().unwrap_or(0.0),
                exposed: exposed.contains(&(volume as u32)),
                effects: Vec::new(),
                volume_index: usize::try_from(volume).ok(),
            }
        })
        .collect();
    let effects = list("effects");
    for effect in effects {
        let group_of = |effect: &Value| usize::try_from(int(effect, "groupConstantIndex")).ok();
        let Some(at) = group_of(effect) else { continue };
        let indices: Vec<usize> = match effect.get("parameterIndices") {
            Some(Value::UInts(indices)) => indices.iter().map(|&i| i as usize).collect(),
            _ => Vec::new(),
        };
        let parameters = indices.iter().map(|&i| values.get(i).copied().unwrap_or(0.0)).collect();
        let target = usize::try_from(int(effect, "sendTargetEffectIndex")).ok().and_then(|e| effects.get(e)).and_then(group_of);
        let Some(group) = groups.get_mut(at) else { continue };
        group.effects.push(MixerEffect { kind: int(effect, "type"), parameters, indices, target });
    }
    let names_hashed: Vec<u32> = match constant.get("exposedParameterNames") {
        Some(Value::UInts(hashes)) => hashes.clone(),
        _ => Vec::new(),
    };
    let exposed = names_hashed.into_iter().zip(exposed.iter().map(|&i| i as usize)).collect();
    let start_snapshot = start.and_then(|id| names.get(&id).cloned()).unwrap_or_default();
    Ok(Mixer { exposed, start_snapshot, groups })
}

/// The bundle of the sounds of moving (`Sounds`, read by the game's
/// `Player.InitAudioController`): for each surface a set of banks
/// (`SurfaceSet`), and the equipment's banks.
pub const MOVEMENT_SOUNDS: &str = "assets/content/audio/prefabs/movement/sounds.bundle";

/// The surfaces a step can be on (`BaseBallistic.ESurfaceSound`), by number.
pub const SURFACES: [&str; 20] = [
    "Concrete", "Metal", "Wood", "Soil", "Grass", "Asphalt", "Glass", "Gravel", "MetalThin", "Puddle", "Slate", "Tile", "WoodThick", "Swamp", "Garbage", "GarbageMetal", "Plastic", "WholeGlass", "WoodThin", "Snow",
];

/// A bank of the game's (`SoundBank`): its figures, and the clips of the
/// one environment the steps are played in (`EnvironmentType.Outdoor`, the
/// first of `Environments`: the player passes it on every call), in its
/// three lists (`DistanceVarity`), each clip by its place in
/// `MovementSounds::clips`.
#[derive(Debug, Clone)]
pub struct Bank {
    pub name: String,
    pub base_volume: f32,
    pub base_pitch: f32,
    pub delta_volume: f32,
    pub delta_pitch: f32,
    pub rolloff: f32,
    /// Its `DistanceBlendOptions.Values`, if it has them: where the blend
    /// value moves from one list to the next.
    pub blend: Option<Vec<f32>>,
    pub lists: [Vec<usize>; 3],
    /// The kind of source it plays from (`SourceType`, BetterAudio's
    /// `AudioSourceGroupType`: 4, `Environment`, if the bank does not say).
    pub source_type: i64,
    /// Whether an effect plays it from afar (`Physical`:
    /// `PlayAtPointDistant`), or limited near others of its kind
    /// (`AllowLimitedPlay`).
    pub physical: bool,
    pub limited: bool,
    /// Its clips' length as the bank keeps it (`ClipLength`: `CustomLength`
    /// where it is not nought, else `_clipLiength`), in seconds.
    pub clip_length: f32,
}

/// The banks of a surface (`SurfaceSet`), in its order.
pub const SURFACE_BANKS: [&str; 9] = ["RunSoundBank", "SprintSoundBank", "StopSoundBank", "LandingSoundBank", "TurnSoundBank", "DuckSoundBank", "ProneSoundBank", "ProneDropSoundBank", "JumpSoundBank"];

/// The equipment's banks (`Sounds`), in that order.
pub const GEAR_BANKS: [&str; 4] = ["Gear", "GearMedium", "GearFast", "BackpackDrop"];

/// A surface's sound sets for the three moves over an obstacle
/// (`SurfaceSet`; footsteps spec, "The vault's sounds"), in the order of
/// the game's `EVaultingSoundType` less its none: a vault, a sprint's
/// vault, a climb.
pub const VAULT_SETS: [&str; 3] = ["VaultSoundSet", "SprintVaultSoundSet", "ClimbSoundSet"];

/// The kit's bank for each of the three moves (`Sounds`), in the same order.
pub const VAULT_GEAR_BANKS: [&str; 3] = ["GearVault", "GearSprintVault", "GearClimb"];

/// The marks of a move at which a set's elements play
/// (`EVaultingSoundState`, by number; its `Gear` is never looked up).
pub const VAULT_MARKS: [&str; 4] = ["Start", "Grab", "Slide", "End"];

/// One mark's sound of a move on a surface (`VaultingSoundElement`): its
/// bank, if it names one (a place in `MovementSounds::banks`), and the
/// range the obstacle's height places its volume in (`VolumeRange`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VaultElement {
    pub bank: Option<usize>,
    pub min_volume: f32,
    pub max_volume: f32,
}

/// A surface's vault sounds: by move (`VAULT_SETS`' order) and mark
/// (`VAULT_MARKS`' order); a mark a set does not list is `None`.
#[derive(Debug, Clone)]
pub struct VaultSounds {
    pub surface: u8,
    pub elements: [[Option<VaultElement>; 4]; 3],
}

/// A surface's banks, by `SURFACE_BANKS`' order, each by its place in
/// `MovementSounds::banks` (a bank missing is `None`; one bank may serve
/// two ways or two surfaces, as the swamp's run serves its crouch too).
#[derive(Debug, Clone)]
pub struct SurfaceSounds {
    pub surface: u8,
    pub banks: Vec<Option<usize>>,
}

/// The sounds of moving as the install keeps them.
#[derive(Debug, Clone, Default)]
pub struct MovementSounds {
    /// Every bank once.
    pub banks: Vec<Bank>,
    pub surfaces: Vec<SurfaceSounds>,
    /// By `GEAR_BANKS`' order, as places in `banks`.
    pub gear: Vec<Option<usize>>,
    /// Each surface's sounds for the moves over an obstacle.
    pub vault: Vec<VaultSounds>,
    /// The kit's banks for those moves, by `VAULT_GEAR_BANKS`' order.
    pub vault_gear: Vec<Option<usize>>,
    /// Every clip a bank names, read or not (one kept in a way not read, or
    /// left unread when only the figures were asked for, is `None`), with
    /// its name.
    pub clips: Vec<(String, Option<Clip>)>,
    /// The switches on the head's clips, by `TOGGLE_SOUNDS`' order (its name
    /// in the `Sounds`, the clip's own name, the clip if read).
    pub toggles: Vec<(&'static str, String, Option<Clip>)>,
}

/// The `Sounds`' clips for a switch on the head turned on and off
/// (`Player.PlayFaceShieldSound` and the rest; inventory spec, "A switch on
/// the head"), and the player's tactical sound (`SwitchHeadlights`: Hold
/// mode's toggles and the head light; tactical-devices spec, "The
/// animation and the sound"), the last.
pub const TOGGLE_SOUNDS: [&str; 7] = ["FaceShieldOn", "FaceShieldOff", "NightVisionOn", "NightVisionOff", "ThermalVisionOn", "ThermalVisionOff", "SwitchHeadlights"];

/// The player's tactical sound's place in `TOGGLE_SOUNDS`.
pub const TACTICAL_SOUND: usize = 6;

impl MovementSounds {
    /// A surface's set, by its number.
    pub fn surface(&self, surface: u8) -> Option<&SurfaceSounds> {
        self.surfaces.iter().find(|set| set.surface == surface)
    }

    /// A surface's bank of the way given (`SURFACE_BANKS`' place).
    pub fn bank(&self, surface: u8, way: usize) -> Option<&Bank> {
        self.surface(surface)?.banks.get(way).copied().flatten().map(|at| &self.banks[at])
    }

    /// A surface's element for a move (`VAULT_SETS`' place) at a mark
    /// (`VAULT_MARKS`' place), if its set lists one.
    pub fn vault_element(&self, surface: u8, kind: usize, mark: usize) -> Option<VaultElement> {
        self.vault.iter().find(|set| set.surface == surface)?.elements.get(kind)?.get(mark).copied().flatten()
    }
}

/// The sounds of moving (`MOVEMENT_SOUNDS`' `Sounds`): every surface's
/// banks and the equipment's, with their clips' samples if `samples`
/// (else only their names), only of the surfaces listed if any are.
pub fn movement_sounds(install: &Install, only: Option<&[u8]>, samples: bool) -> Result<MovementSounds> {
    use crate::unity::{class, extract};
    let mut setups = VorbisSetups::new(install);
    let mut assets = crate::unity::Assets::new(install.clone())?;
    let files = assets.load(MOVEMENT_SOUNDS)?;
    let sounds = files
        .iter()
        .flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR).collect::<Vec<_>>())
        .find(|&obj| extract::script_class(&assets, obj).as_deref() == Some("Sounds"))
        .context("no Sounds in the movement's bundle")?;
    let value = assets.read(sounds)?;
    let mut out = MovementSounds::default();
    let mut reader = BankReader::new(samples);
    for set in value.get("Sets").and_then(Value::as_array).unwrap_or(&[]) {
        let surface = set.get("Surface").and_then(Value::as_i64).unwrap_or(0) as u8;
        if only.is_some_and(|only| !only.contains(&surface)) {
            continue;
        }
        let mut banks = Vec::new();
        for name in SURFACE_BANKS {
            banks.push(reader.bank_at(&assets, &mut setups, sounds.file, set.get(name), &mut out.banks, &mut out.clips)?);
        }
        out.surfaces.push(SurfaceSounds { surface, banks });
        // The three moves' sets: each a list of elements, a mark's number
        // each, with its bank and its volume's range.
        let mut elements: [[Option<VaultElement>; 4]; 3] = Default::default();
        for (kind, name) in VAULT_SETS.iter().enumerate() {
            for element in set.get(name).and_then(|set| set.get("VaultingBanks")).and_then(Value::as_array).unwrap_or(&[]) {
                let mark = element.get("SoundState").and_then(Value::as_i64).unwrap_or(-1);
                let Some(slot) = usize::try_from(mark).ok().and_then(|mark| elements[kind].get_mut(mark)) else { continue };
                let bank = reader.bank_at(&assets, &mut setups, sounds.file, element.get("SoundBank"), &mut out.banks, &mut out.clips)?;
                let range = |name: &str, or: f32| element.get("VolumeRange").and_then(|range| range.get(name)).and_then(Value::as_f32).unwrap_or(or);
                *slot = Some(VaultElement { bank, min_volume: range("MinVolume", 0.1), max_volume: range("MaxVolume", 0.7) });
            }
        }
        out.vault.push(VaultSounds { surface, elements });
    }
    for name in GEAR_BANKS {
        let bank = reader.bank_at(&assets, &mut setups, sounds.file, value.get(name), &mut out.banks, &mut out.clips)?;
        out.gear.push(bank);
    }
    for name in VAULT_GEAR_BANKS {
        let bank = reader.bank_at(&assets, &mut setups, sounds.file, value.get(name), &mut out.banks, &mut out.clips)?;
        out.vault_gear.push(bank);
    }
    let mut unread = std::mem::take(&mut reader.unread);
    for name in TOGGLE_SOUNDS {
        let Some(at) = value.get(name).and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(sounds.file, p)) else { continue };
        let clip = assets.read(at)?;
        let clip_name = clip.get("m_Name").and_then(Value::as_str).unwrap_or_default().to_owned();
        let read = if samples {
            let resource = |source: &str, offset: usize, size: usize| -> Result<Vec<u8>> { Ok(assets.resource(source, offset, size)?.to_vec()) };
            match clip_of(&clip, &resource, &mut setups) {
                Ok(clip) => Some(clip),
                Err(e) => {
                    unread.push(format!("{e:#}"));
                    None
                }
            }
        } else {
            None
        };
        out.toggles.push((name, clip_name, read));
    }
    if !unread.is_empty() {
        eprintln!("{} of the sounds of moving are not read and are silent: {}", unread.len(), unread.join("; "));
    }
    Ok(out)
}

/// Banks read from loaded files (`SoundBank`), each once, and their clips
/// each once, with their samples if asked for: what could not be read kept
/// to be told.
struct BankReader {
    samples: bool,
    clip_places: HashMap<crate::unity::ObjRef, usize>,
    bank_places: HashMap<crate::unity::ObjRef, usize>,
    unread: Vec<String>,
}

impl BankReader {
    fn new(samples: bool) -> Self {
        Self { samples, clip_places: HashMap::new(), bank_places: HashMap::new(), unread: Vec::new() }
    }

    /// A bank by its pointer from `from`'s file, put in `banks` with its
    /// clips (in `clips`) if it is not there yet: its place there. Only its
    /// first environment's clips (`EnvironmentType.Outdoor`).
    #[allow(clippy::too_many_arguments)]
    fn bank_at(&mut self, assets: &crate::unity::Assets, setups: &mut VorbisSetups, from: usize, pointer: Option<&Value>, banks: &mut Vec<Bank>, clips: &mut Vec<(String, Option<Clip>)>) -> Result<Option<usize>> {
        let Some(at) = pointer.and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(from, p)) else { return Ok(None) };
        if let Some(&place) = self.bank_places.get(&at) {
            return Ok(Some(place));
        }
        let bank = assets.read(at)?;
        let number = |name: &str, or: f32| bank.get(name).and_then(Value::as_f32).unwrap_or(or);
        let blend = bank
            .get("BlendOptions")
            .and_then(Value::as_pptr)
            .and_then(|p| assets.resolve_loaded(at.file, p))
            .and_then(|options| assets.read(options).ok())
            .and_then(|options| options.get("Values").and_then(Value::as_floats).map(<[f32]>::to_vec));
        let mut lists: [Vec<usize>; 3] = Default::default();
        let first = bank.get("Environments").and_then(Value::as_array).and_then(|all| all.first());
        let varieties = first.and_then(|e| e.get("Clips")).and_then(Value::as_array).unwrap_or(&[]);
        for (list, variety) in lists.iter_mut().zip(varieties) {
            for pointer in variety.get("Clips").and_then(Value::as_array).unwrap_or(&[]) {
                let Some(clip) = pointer.as_pptr().and_then(|p| assets.resolve_loaded(at.file, p)) else { continue };
                let place = match self.clip_places.get(&clip) {
                    Some(&place) => place,
                    None => {
                        let value = assets.read(clip)?;
                        let name = value.get("m_Name").and_then(Value::as_str).unwrap_or_default().to_owned();
                        let read = if self.samples {
                            let resource = |source: &str, offset: usize, size: usize| -> Result<Vec<u8>> { Ok(assets.resource(source, offset, size)?.to_vec()) };
                            match clip_of(&value, &resource, setups) {
                                Ok(clip) => Some(clip),
                                Err(e) => {
                                    self.unread.push(format!("{e:#}"));
                                    None
                                }
                            }
                        } else {
                            None
                        };
                        clips.push((name, read));
                        self.clip_places.insert(clip, clips.len() - 1);
                        clips.len() - 1
                    }
                };
                list.push(place);
            }
        }
        banks.push(Bank {
            name: bank.get("m_Name").and_then(Value::as_str).unwrap_or_default().to_owned(),
            base_volume: number("BaseVolume", 1.0),
            base_pitch: number("BasePitch", 1.0),
            delta_volume: number("DeltaVolume", 0.0),
            delta_pitch: number("DeltaPitch", 0.0),
            rolloff: number("Rolloff", 100.0),
            blend,
            lists,
            source_type: bank.get("SourceType").and_then(Value::as_i64).unwrap_or(4),
            physical: bank.get("Physical").and_then(Value::as_bool).unwrap_or(false),
            limited: bank.get("AllowLimitedPlay").and_then(Value::as_bool).unwrap_or(false),
            clip_length: match number("CustomLength", 0.0) {
                custom if custom != 0.0 => custom,
                _ => number("_clipLiength", 0.0),
            },
        });
        self.bank_places.insert(at, banks.len() - 1);
        Ok(Some(banks.len() - 1))
    }
}

/// The weapon's mount sound (`WeaponSounds.VerticalMountSounds`, a `SoundBank`
/// in `WEAPON_SOUNDS`: `WeaponSoundPlayer.PlayMountSound`, the approach's time
/// after any mount; mounting spec, "Sounds"): the bank and its clips, read
/// whole.
pub fn mount_sounds(install: &Install) -> Result<(Bank, Vec<(String, Clip)>)> {
    use crate::unity::{class, extract};
    let mut setups = VorbisSetups::new(install);
    let mut assets = crate::unity::Assets::new(install.clone())?;
    let files = assets.load(WEAPON_SOUNDS)?;
    let sounds = files
        .iter()
        .flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR).collect::<Vec<_>>())
        .find(|&obj| extract::script_class(&assets, obj).as_deref() == Some("WeaponSounds"))
        .context("no WeaponSounds in its bundle")?;
    let value = assets.read(sounds)?;
    let mut reader = BankReader::new(true);
    let (mut banks, mut clips) = (Vec::new(), Vec::new());
    let at = reader.bank_at(&assets, &mut setups, sounds.file, value.get("VerticalMountSounds"), &mut banks, &mut clips)?.context("no VerticalMountSounds")?;
    let bank = banks.swap_remove(at);
    let read = bank.lists[0].iter().filter_map(|&place| clips.get(place).and_then(|(name, clip)| Some((name.clone(), clip.clone()?)))).collect();
    Ok((bank, read))
}

/// The ringing of a contusion (`Sounds.TinnitusSound`, in `MOVEMENT_SOUNDS`:
/// `Player.TryStartContusion`'s clip), read whole.
pub fn tinnitus_sound(install: &Install) -> Result<Clip> {
    movement_clip(install, "TinnitusSound")
}

/// The folder of the voices' phrase bundles.
pub const PHRASES: &str = "assets/content/audio/phrases";

/// One clip of a voice's bank (`TaggedClip`): the tags it answers to
/// (`ETagStatus` bits; nought answers to any), its volume, its falloff in
/// metres, its length as the bank keeps it, and the sound itself.
#[derive(Debug, Clone)]
pub struct PhraseClip {
    pub name: String,
    pub mask: u32,
    pub volume: f32,
    pub falloff: f32,
    pub length: f32,
    pub clip: Option<Clip>,
}

/// One bank of a voice (`TagBank`): what it is said for (`EPhraseTrigger`),
/// how much it matters against what is being said, whether its clips'
/// tags count, and its clips (the spread groups' together, as the bank
/// keeps them).
#[derive(Debug, Clone)]
pub struct PhraseBank {
    pub trigger: i32,
    pub importance: i32,
    pub ignore_tags: bool,
    pub clips: Vec<PhraseClip>,
}

/// A voice's banks for the triggers asked for (`Voice.Banks`), in the
/// voice's order, and the bundle it was read from.
#[derive(Debug, Clone, Default)]
pub struct VoiceBanks {
    pub name: String,
    pub bundle: String,
    pub banks: Vec<PhraseBank>,
}

/// A voice's banks by its name (`CustomizationSolver.GetVoice(...).Name`,
/// `Bear_2_Eng`): its phrase bundle is the name in small letters and
/// `_voice.bundle` for most (`InGameBundles.TakePhrasePath`); where that
/// bundle is not there or holds another voice, every phrase bundle is
/// looked through for the `Voice` of that name. Only the banks of
/// `triggers` are read; their clips' sounds with `samples`.
pub fn voice_banks(install: &Install, name: &str, triggers: &[i32], samples: bool) -> Result<VoiceBanks> {
    use crate::unity::{class, extract};
    let guess = format!("{PHRASES}/{}_voice.bundle", name.to_lowercase());
    let mut keys = vec![guess.clone()];
    if let Ok(dir) = std::fs::read_dir(install.bundle_path(PHRASES)) {
        let mut others: Vec<String> = dir.filter_map(|e| e.ok()).filter_map(|e| e.file_name().into_string().ok()).filter(|f| f.ends_with(".bundle")).map(|f| format!("{PHRASES}/{f}")).filter(|k| *k != guess).collect();
        others.sort();
        keys.extend(others);
    }
    for key in keys {
        if !install.bundle_path(&key).exists() {
            continue;
        }
        let mut setups = VorbisSetups::new(install);
        let mut assets = crate::unity::Assets::new(install.clone())?;
        let Ok(files) = assets.load(&key) else { continue };
        let voice = files.iter().flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR).collect::<Vec<_>>()).find(|&obj| {
            extract::script_class(&assets, obj).as_deref() == Some("Voice") && assets.read(obj).ok().and_then(|v| v.get("Name").and_then(Value::as_str).map(|n| n == name)).unwrap_or(false)
        });
        let Some(voice) = voice else { continue };
        let value = assets.read(voice)?;
        let mut out = VoiceBanks { name: name.to_owned(), bundle: key.clone(), banks: Vec::new() };
        for pptr in value.get("Banks").and_then(Value::as_array).unwrap_or(&[]).iter().filter_map(Value::as_pptr) {
            let Some(at) = assets.resolve_loaded(voice.file, pptr) else { continue };
            let bank = assets.read(at)?;
            let trigger = bank.get("Trigger").and_then(Value::as_i64).unwrap_or(-1) as i32;
            if !triggers.contains(&trigger) {
                continue;
            }
            let mut clips = Vec::new();
            for tagged in bank.get("Clips").and_then(Value::as_array).unwrap_or(&[]) {
                let f = |k: &str| tagged.get(k).and_then(Value::as_f32).unwrap_or(0.0);
                let found = tagged.get("Clip").and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(at.file, p));
                let (clip_name, clip) = match found {
                    Some(obj) => {
                        let read = assets.read(obj)?;
                        let clip_name = read.get("m_Name").and_then(Value::as_str).unwrap_or_default().to_owned();
                        let clip = if samples {
                            let resource = |source: &str, offset: usize, size: usize| -> Result<Vec<u8>> { Ok(assets.resource(source, offset, size)?.to_vec()) };
                            match clip_of(&read, &resource, &mut setups) {
                                Ok(clip) => Some(clip),
                                Err(e) => {
                                    eprintln!("the voice {name}'s {clip_name} could not be read; it is silent: {e:#}");
                                    None
                                }
                            }
                        } else {
                            None
                        };
                        (clip_name, clip)
                    }
                    None => (String::new(), None),
                };
                clips.push(PhraseClip {
                    name: clip_name,
                    mask: tagged.get("Mask").and_then(Value::as_i64).unwrap_or(0) as u32,
                    volume: f("Volume"),
                    falloff: f("Falloff"),
                    length: f("Length"),
                    clip,
                });
            }
            out.banks.push(PhraseBank {
                trigger,
                importance: bank.get("Importance").and_then(Value::as_i64).unwrap_or(0) as i32,
                ignore_tags: bank.get("IgnoreTags").and_then(|v| v.as_bool().or_else(|| v.as_i64().map(|i| i != 0))).unwrap_or(false),
                clips,
            });
        }
        return Ok(out);
    }
    bail!("no phrase bundle holds the voice {name}")
}

/// A clip the movement's `Sounds` names (`TinnitusSound`, `FractureSound`).
fn movement_clip(install: &Install, field: &str) -> Result<Clip> {
    use crate::unity::{class, extract};
    let mut setups = VorbisSetups::new(install);
    let mut assets = crate::unity::Assets::new(install.clone())?;
    let files = assets.load(MOVEMENT_SOUNDS)?;
    let sounds = files
        .iter()
        .flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR).collect::<Vec<_>>())
        .find(|&obj| extract::script_class(&assets, obj).as_deref() == Some("Sounds"))
        .context("no Sounds in the movement's bundle")?;
    let value = assets.read(sounds)?;
    let at = value.get(field).and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(sounds.file, p)).with_context(|| format!("the Sounds name no {field}"))?;
    let clip = assets.read(at)?;
    let resource = |source: &str, offset: usize, size: usize| -> Result<Vec<u8>> { Ok(assets.resource(source, offset, size)?.to_vec()) };
    clip_of(&clip, &resource, &mut setups)
}

/// A bone's breaking (`Sounds.FractureSound`, `bone_break`), which a
/// fracture plays as it starts (`Player.OnHealthEffectAdded`).
pub fn fracture_sound(install: &Install) -> Result<Clip> {
    movement_clip(install, "FractureSound")
}

/// The bundle of the world's effects (`Effects`: each named effect's
/// particles, decals and sounds, `EffectsArray`).
pub const EFFECTS: &str = "assets/systems/effects/particlesystems/effects.bundle";

/// Effects' sounds (`Effects.Effect.Sound`, the bank an effect plays where
/// it is not seen in first person): each effect asked for by its name with
/// its bank's place in `banks` (none where it has no sound or is not
/// there), and the banks' clips.
#[derive(Debug, Clone, Default)]
pub struct EffectSounds {
    pub banks: Vec<Bank>,
    pub effects: Vec<(String, Option<usize>)>,
    pub clips: Vec<(String, Option<Clip>)>,
    /// The effects' other banks (`AdditionalSoundEffects`, in order: the
    /// first a ricochet's), each by its place in `banks`.
    pub additional: Vec<Option<usize>>,
}

impl EffectSounds {
    /// An effect's bank by its name.
    pub fn bank(&self, name: &str) -> Option<&Bank> {
        self.effects.iter().find(|(effect, _)| effect == name).and_then(|(_, at)| *at).map(|at| &self.banks[at])
    }
}

/// The sounds of the effects named (`EFFECTS`), with their clips' samples
/// if `samples`.
pub fn effect_sounds(install: &Install, names: &[String], samples: bool) -> Result<EffectSounds> {
    use crate::unity::{class, extract};
    let mut setups = VorbisSetups::new(install);
    let mut assets = crate::unity::Assets::new(install.clone())?;
    let files = assets.load(EFFECTS)?;
    let effects = files
        .iter()
        .flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR).collect::<Vec<_>>())
        .find(|&obj| extract::script_class(&assets, obj).as_deref() == Some("Effects"))
        .context("no Effects in its bundle")?;
    let value = assets.read(effects)?;
    let all = value.get("EffectsArray").and_then(Value::as_array).context("the effects have no EffectsArray")?;
    let mut out = EffectSounds::default();
    let mut reader = BankReader::new(samples);
    for name in names {
        let entry = all.iter().find(|effect| effect.get("Name").and_then(Value::as_str) == Some(name.as_str()));
        let bank = match entry {
            Some(effect) => reader.bank_at(&assets, &mut setups, effects.file, effect.get("Sound"), &mut out.banks, &mut out.clips)?,
            None => None,
        };
        out.effects.push((name.clone(), bank));
    }
    for extra in value.get("AdditionalSoundEffects").and_then(Value::as_array).unwrap_or(&[]) {
        let bank = reader.bank_at(&assets, &mut setups, effects.file, Some(extra), &mut out.banks, &mut out.clips)?;
        out.additional.push(bank);
    }
    if !reader.unread.is_empty() {
        eprintln!("{} of the effects' sounds are not read and are silent: {}", reader.unread.len(), reader.unread.join("; "));
    }
    Ok(out)
}

/// The bundle of the weapons' sounds as they land (`ItemDropSounds`, which
/// the game's `BetterAudio` loads as it begins).
pub const ITEM_DROP_SOUNDS: &str = "assets/content/audio/prefabs/items/itemdropsounds.bundle";

/// Each surface's banks for the kinds of weapon that sound as they land
/// (`ItemDropSurfaceSet`), in the order of the game's `EItemDropSoundType`
/// less its none: a pistol, a submachine gun, a rifle.
pub const DROP_KINDS: [&str; 3] = ["PistolDropSoundBank", "SubGunDropSoundBank", "RiffleDropSoundBank"];

/// A template's `DropSoundType` as the game numbers it (`EItemDropSoundType`):
/// none 0, a pistol 1, a submachine gun 2, a rifle 3.
pub fn drop_kind(name: &str) -> u8 {
    match name {
        "Pistol" => 1,
        "SubMachineGun" => 2,
        "Rifle" => 3,
        _ => 0,
    }
}

/// The weapons' landing sounds (`ITEM_DROP_SOUNDS`' `ItemDropSounds`).
#[derive(Debug, Clone, Default)]
pub struct DropSounds {
    pub banks: Vec<Bank>,
    /// The volume by the landing's energy (`EnergyToVolumeCurve`).
    pub volume: crate::curve::Curve,
    /// The surface taken for one with no set (`DefaultSurfaceSound`).
    pub default_surface: u8,
    /// Each surface's banks by `DROP_KINDS`' order, as places in `banks`;
    /// a surface listed twice keeps its first set, as the game's map does.
    pub surfaces: Vec<(u8, [Option<usize>; 3])>,
    pub clips: Vec<(String, Option<Clip>)>,
}

impl DropSounds {
    /// The bank a weapon of a kind (`drop_kind`'s number) plays landing on
    /// a surface (`ItemDropSounds.GetSoundBank`): its surface's set, or the
    /// default surface's where it has none; nothing for a kind of none, a
    /// set without that bank, or no set for the default surface either
    /// (the game's lookup fails there).
    pub fn bank(&self, surface: u8, kind: u8) -> Option<usize> {
        let set = |surface: u8| self.surfaces.iter().find(|(s, _)| *s == surface);
        let (_, banks) = set(surface).or_else(|| set(self.default_surface))?;
        banks.get(usize::from(kind).checked_sub(1)?).copied().flatten()
    }
}

/// The weapons' landing sounds, with their clips' samples if `samples`.
pub fn drop_sounds(install: &Install, samples: bool) -> Result<DropSounds> {
    use crate::unity::{class, extract};
    let mut setups = VorbisSetups::new(install);
    let mut assets = crate::unity::Assets::new(install.clone())?;
    let files = assets.load(ITEM_DROP_SOUNDS)?;
    let sounds = files
        .iter()
        .flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR).collect::<Vec<_>>())
        .find(|&obj| extract::script_class(&assets, obj).as_deref() == Some("ItemDropSounds"))
        .context("no ItemDropSounds in its bundle")?;
    let value = assets.read(sounds)?;
    let mut out = DropSounds {
        volume: crate::player::curve(value.req("EnergyToVolumeCurve")?)?,
        default_surface: value.get("DefaultSurfaceSound").and_then(Value::as_i64).unwrap_or(0) as u8,
        ..DropSounds::default()
    };
    let mut reader = BankReader::new(samples);
    for set in value.get("SurfaceSets").and_then(Value::as_array).unwrap_or(&[]) {
        let surface = set.get("Surface").and_then(Value::as_i64).unwrap_or(0) as u8;
        let mut banks = [None; 3];
        for (bank, name) in banks.iter_mut().zip(DROP_KINDS) {
            *bank = reader.bank_at(&assets, &mut setups, sounds.file, set.get(name), &mut out.banks, &mut out.clips)?;
        }
        if !out.surfaces.iter().any(|(s, _)| *s == surface) {
            out.surfaces.push((surface, banks));
        }
    }
    if !reader.unread.is_empty() {
        eprintln!("{} of the weapons' landing sounds are not read and are silent: {}", reader.unread.len(), reader.unread.join("; "));
    }
    Ok(out)
}

/// The calibres' classes of a spent case's sounds (`ECaliber`, a round's
/// `AmmoPoolObject._caliber`), by number, as `WeaponSounds` names them.
pub const SHELL_CALIBERS: [&str; 4] = ["9mm", "12cal", "556mm", "Heavy"];

/// The surfaces a spent case has sounds for, as `WeaponSounds` names them
/// (which surface plays which: `trp_sim::shells::sound_surface`).
pub const SHELL_SURFACES: [&str; 5] = ["Concrete", "Metal", "Soil", "Wood", "Plastic"];

/// The spent cases' sounds (`WEAPON_SOUNDS`' `Shell<calibre><surface>`):
/// every bank once, by calibre and surface (`SHELL_CALIBERS`,
/// `SHELL_SURFACES`), with their clips.
#[derive(Debug, Clone, Default)]
pub struct ShellSounds {
    pub banks: Vec<Bank>,
    pub by: [[Option<usize>; 5]; 4],
    pub clips: Vec<(String, Option<Clip>)>,
}

impl ShellSounds {
    /// The bank of a calibre's class and a surface of `SHELL_SURFACES`.
    pub fn bank(&self, caliber: u8, surface: usize) -> Option<usize> {
        *self.by.get(caliber as usize)?.get(surface)?
    }
}

/// The spent cases' sounds (`WeaponSounds`, `Firearms.PlayShellSound`),
/// with their clips' samples if `samples`.
pub fn shell_sounds(install: &Install, samples: bool) -> Result<ShellSounds> {
    use crate::unity::{class, extract};
    let mut setups = VorbisSetups::new(install);
    let mut assets = crate::unity::Assets::new(install.clone())?;
    let files = assets.load(WEAPON_SOUNDS)?;
    let sounds = files
        .iter()
        .flat_map(|&file| assets.objects_of(file, class::MONO_BEHAVIOUR).collect::<Vec<_>>())
        .find(|&obj| extract::script_class(&assets, obj).as_deref() == Some("WeaponSounds"))
        .context("no WeaponSounds in its bundle")?;
    let value = assets.read(sounds)?;
    let mut out = ShellSounds::default();
    let mut reader = BankReader::new(samples);
    for (c, caliber) in SHELL_CALIBERS.iter().enumerate() {
        for (s, surface) in SHELL_SURFACES.iter().enumerate() {
            out.by[c][s] = reader.bank_at(&assets, &mut setups, sounds.file, value.get(&format!("Shell{caliber}{surface}")), &mut out.banks, &mut out.clips)?;
        }
    }
    if !reader.unread.is_empty() {
        eprintln!("{} of the spent cases' sounds are not read and are silent: {}", reader.unread.len(), reader.unread.join("; "));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bank of one Vorbis sample, made as FMOD lays one out: the header,
    /// the sample's 64 bits, a frequency chunk and a Vorbis chunk with its
    /// setup header's CRC32, then two packets after their sizes.
    #[test]
    fn a_vorbis_bank_is_read_as_fmod_lays_it_out() {
        let packets: [&[u8]; 2] = [&[1, 2, 3], &[4, 5]];
        let mut data = Vec::new();
        for packet in packets {
            data.extend_from_slice(&(packet.len() as u16).to_le_bytes());
            data.extend_from_slice(packet);
        }
        // Mono (0), 44.1 kHz (8), offset 0, 1234 frames, chunks after.
        let mode: u64 = 1 | (8 << 1) | (1234u64 << 34);
        let mut chunks = Vec::new();
        // A frequency chunk of 4 bytes (kind 2) that says 48000, another follows.
        chunks.extend_from_slice(&(1u32 | (4 << 1) | (2 << 25)).to_le_bytes());
        chunks.extend_from_slice(&48000u32.to_le_bytes());
        // The Vorbis chunk (kind 11): the CRC32, the last chunk.
        chunks.extend_from_slice(&((4u32 << 1) | (11 << 25)).to_le_bytes());
        chunks.extend_from_slice(&0xdead_beefu32.to_le_bytes());
        let headers = 8 + chunks.len();
        let mut bank = b"FSB5".to_vec();
        for word in [1u32, 1, headers as u32, 0, data.len() as u32, FSB5_VORBIS] {
            bank.extend_from_slice(&word.to_le_bytes());
        }
        bank.resize(0x3c, 0);
        bank.extend_from_slice(&mode.to_le_bytes());
        bank.extend_from_slice(&chunks);
        bank.extend_from_slice(&data);
        let sample = fsb5_vorbis(&bank).expect("read");
        assert_eq!((sample.channels, sample.frequency, sample.frames, sample.setup_crc), (1, 48000, 1234, 0xdead_beef));
        assert_eq!(sample.packets, vec![vec![1, 2, 3], vec![4, 5]]);
    }

    #[test]
    fn a_mark_names_its_sound_with_or_without_snd() {
        let element = |event: &str| SoundElement { event: event.into(), rolloff: 30, volume: 1.0, clips: Vec::new() };
        let sounds = HandsSounds { player: "BaseSoundPlayer".into(), main: Vec::new(), additional: vec![element("SndDraw"), element("MagIn")], shots: None };
        assert_eq!(sounds.for_mark("Draw").map(|e| e.event.as_str()), Some("SndDraw"));
        assert_eq!(sounds.for_mark("MagIn").map(|e| e.event.as_str()), Some("MagIn"));
        assert!(sounds.for_mark("Holster").is_none());
    }
}
