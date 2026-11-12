//! Audio clips: FSB5 samples, Vorbis setups and decoding, IMA steps.

use super::*;

/// The engine's class number of an audio clip.
pub(super) const AUDIO_CLIP: i32 = 83;

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
    pub(super) path: std::path::PathBuf,
    engine: Option<Vec<u8>>,
    pub(super) found: HashMap<u32, Vec<u8>>,
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
