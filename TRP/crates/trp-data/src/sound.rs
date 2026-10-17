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
fn clip_of(clip: &Value, bank: &dyn Fn(&str, usize, usize) -> Result<Vec<u8>>, setups: &mut VorbisSetups) -> Result<Clip> {
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

/// The interface's sounds of the kinds asked (`EUISoundType`'s numbers), by
/// kind: the Resource `audio/uisoundswrapper` (`UISoundsWrapper`), whose
/// fields after a script's own begin with its list of a kind and a clip's
/// pointer each (`_UIAudioClips`), the clips read from `resources.assets`
/// by an audio clip's type tree borrowed from the things' bundle, their
/// banks from the file their resource names beside it.
pub fn ui_sounds(install: &Install, kinds: &[i32]) -> Result<HashMap<i32, Clip>> {
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
    Ok(out)
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
            }
        })
        .collect();
    for effect in list("effects") {
        let Some(group) = usize::try_from(int(effect, "groupConstantIndex")).ok().and_then(|g| groups.get_mut(g)) else { continue };
        let parameters = match effect.get("parameterIndices") {
            Some(Value::UInts(indices)) => indices.iter().map(|&i| values.get(i as usize).copied().unwrap_or(0.0)).collect(),
            _ => Vec::new(),
        };
        group.effects.push(MixerEffect { kind: int(effect, "type"), parameters });
    }
    let start_snapshot = start.and_then(|id| names.get(&id).cloned()).unwrap_or_default();
    Ok(Mixer { start_snapshot, groups })
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
}

/// The banks of a surface (`SurfaceSet`), in its order.
pub const SURFACE_BANKS: [&str; 9] = ["RunSoundBank", "SprintSoundBank", "StopSoundBank", "LandingSoundBank", "TurnSoundBank", "DuckSoundBank", "ProneSoundBank", "ProneDropSoundBank", "JumpSoundBank"];

/// The equipment's banks (`Sounds`), in that order.
pub const GEAR_BANKS: [&str; 4] = ["Gear", "GearMedium", "GearFast", "BackpackDrop"];

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
    /// Every clip a bank names, read or not (one kept in a way not read, or
    /// left unread when only the figures were asked for, is `None`), with
    /// its name.
    pub clips: Vec<(String, Option<Clip>)>,
}

impl MovementSounds {
    /// A surface's set, by its number.
    pub fn surface(&self, surface: u8) -> Option<&SurfaceSounds> {
        self.surfaces.iter().find(|set| set.surface == surface)
    }

    /// A surface's bank of the way given (`SURFACE_BANKS`' place).
    pub fn bank(&self, surface: u8, way: usize) -> Option<&Bank> {
        self.surface(surface)?.banks.get(way).copied().flatten().map(|at| &self.banks[at])
    }
}

/// The sounds of moving (`MOVEMENT_SOUNDS`' `Sounds`): every surface's
/// banks and the equipment's, with their clips' samples if `samples`
/// (else only their names), only of the surfaces listed if any are.
pub fn movement_sounds(install: &Install, only: Option<&[u8]>, samples: bool) -> Result<MovementSounds> {
    use crate::unity::{class, extract, ObjRef};
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
    let mut clip_places: HashMap<ObjRef, usize> = HashMap::new();
    let mut bank_places: HashMap<ObjRef, usize> = HashMap::new();
    let mut unread = Vec::new();
    // A bank by its pointer from `from`'s file, put in `out` with its
    // clips if it is not there yet: its place there.
    let mut bank_at = |from: usize, pointer: Option<&Value>, out: &mut MovementSounds| -> Result<Option<usize>> {
        let Some(at) = pointer.and_then(Value::as_pptr).and_then(|p| assets.resolve_loaded(from, p)) else { return Ok(None) };
        if let Some(&place) = bank_places.get(&at) {
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
                let place = match clip_places.get(&clip) {
                    Some(&place) => place,
                    None => {
                        let value = assets.read(clip)?;
                        let name = value.get("m_Name").and_then(Value::as_str).unwrap_or_default().to_owned();
                        let read = if samples {
                            let resource = |source: &str, offset: usize, size: usize| -> Result<Vec<u8>> { Ok(assets.resource(source, offset, size)?.to_vec()) };
                            match clip_of(&value, &resource, &mut setups) {
                                Ok(clip) => Some(clip),
                                Err(e) => {
                                    unread.push(format!("{e:#}"));
                                    None
                                }
                            }
                        } else {
                            None
                        };
                        out.clips.push((name, read));
                        clip_places.insert(clip, out.clips.len() - 1);
                        out.clips.len() - 1
                    }
                };
                list.push(place);
            }
        }
        out.banks.push(Bank {
            name: bank.get("m_Name").and_then(Value::as_str).unwrap_or_default().to_owned(),
            base_volume: number("BaseVolume", 1.0),
            base_pitch: number("BasePitch", 1.0),
            delta_volume: number("DeltaVolume", 0.0),
            delta_pitch: number("DeltaPitch", 0.0),
            rolloff: number("Rolloff", 100.0),
            blend,
            lists,
        });
        bank_places.insert(at, out.banks.len() - 1);
        Ok(Some(out.banks.len() - 1))
    };
    for set in value.get("Sets").and_then(Value::as_array).unwrap_or(&[]) {
        let surface = set.get("Surface").and_then(Value::as_i64).unwrap_or(0) as u8;
        if only.is_some_and(|only| !only.contains(&surface)) {
            continue;
        }
        let mut banks = Vec::new();
        for name in SURFACE_BANKS {
            banks.push(bank_at(sounds.file, set.get(name), &mut out)?);
        }
        out.surfaces.push(SurfaceSounds { surface, banks });
    }
    for name in GEAR_BANKS {
        let bank = bank_at(sounds.file, value.get(name), &mut out)?;
        out.gear.push(bank);
    }
    if !unread.is_empty() {
        eprintln!("{} of the sounds of moving are not read and are silent: {}", unread.len(), unread.join("; "));
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
