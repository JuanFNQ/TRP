//! Tests of the sounds' reading: a Vorbis bank, a mark's sound and the headphones' settings.

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

/// The two fades the headphones use: `EaseIn` a square, ending on 1;
/// `InverseExponential` 1 - 2^(-10 t), a half by a tenth of the time,
/// ending a 1024th short of 1. Every fade begins at 0.
#[test]
fn the_headphones_fades_are_the_game_s_curves() {
    assert_eq!((FadeType::EaseIn.ease(0.5), FadeType::EaseIn.ease(1.0)), (0.25, 1.0));
    assert!((FadeType::InverseExponential.ease(0.1) - 0.5).abs() < 1e-6);
    assert!((FadeType::InverseExponential.ease(1.0) - (1.0 - 1.0 / 1024.0)).abs() < 1e-7);
    for fade in FadeType::ALL.into_iter().filter(|fade| *fade != FadeType::Exponential && *fade != FadeType::Logistic) {
        assert!(fade.ease(0.0).abs() < 1e-6, "{fade:?}");
    }
}

/// The backend's figures by name or number, the class's own where it
/// leaves one out.
#[test]
fn the_headphones_settings_are_read_by_name_or_number() {
    let given = serde_json::json!({ "AudioSettings": { "HeadphonesSettings": { "FadeDuration": 0.5, "FadeIn": "Linear", "FadeOut": 7 } } });
    assert_eq!(headphones_settings(&given), HeadphonesSettings { fade_duration: 0.5, fade_in: FadeType::Linear, fade_out: FadeType::SmoothStep });
    assert_eq!(headphones_settings(&serde_json::Value::Null), HeadphonesSettings::default());
    let partial = serde_json::json!({ "AudioSettings": { "HeadphonesSettings": { "FadeIn": "EaseOut" } } });
    assert_eq!(headphones_settings(&partial), HeadphonesSettings { fade_in: FadeType::EaseOut, ..HeadphonesSettings::default() });
}
