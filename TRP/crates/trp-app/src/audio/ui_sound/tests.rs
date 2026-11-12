//! Tests of the interface's and the things' sounds.
use super::*;

#[test]
fn each_interface_sound_is_named_as_the_game_s_enum_at_its_number() {
    // `EUISoundType` in the game's order, to the last the app plays.
    const GAME: [&str; 41] = [
        "BackpackClose", "BackpackOpen", "ButtonBottomBarClick", "ButtonClick", "ButtonOver", "ChatSelect", "ErrorMessage", "InsuranceInsured",
        "InsuranceItemOnInsure", "MenuCheckBox", "MenuContextMenu", "MenuDropdown", "MenuDropdownSelect", "MenuEscape", "MenuInspectorWindowClose",
        "MenuInspectorWindowOpen", "MenuInstallMag", "MenuInstallModFunc", "MenuInstallModGear", "MenuInstallModVital", "MenuMapTag", "MenuModdingClose",
        "MenuModdingOpen", "MenuOpenContainer", "MenuStock", "MenuTraderPress", "QuestCompleted", "QuestFailed", "QuestFinished", "QuestStarted",
        "QuestSubTrackComplete", "RepairComplete", "TradeOperationComplete", "MenuWeaponAssemble", "MenuWeaponDisassemble", "TacticalClothingBuy",
        "TacticalClothingApply", "GeneratorTurnOn", "GeneratorTurnOff", "PlayerIsDead", "MalfunctionExamined",
    ];
    for ui in Ui::ALL {
        assert_eq!(ui.game_name(), GAME[ui as usize], "{ui:?}");
    }
}

/// A stereo clip of two decaying tones, one on each channel, at `RATE`.
fn clip() -> Vec<[f32; 2]> {
    (0..4410)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let tone = |hz: f32, fall: f32| (t * hz * std::f32::consts::TAU).sin() * (-t * fall).exp() * 0.5;
            [tone(3000.0, 30.0), tone(1700.0, 20.0)]
        })
        .collect()
}

/// The clip at the output's rate, by straight lines between frames.
fn at_48k(frames: &[[f32; 2]]) -> Vec<[f32; 2]> {
    let count = frames.len() * 48_000 / RATE as usize;
    (0..count)
        .map(|i| {
            let t = i as f64 * RATE as f64 / 48_000.0;
            let (k, f) = (t as usize, (t - (t as usize) as f64) as f32);
            let a = frames[k.min(frames.len() - 1)];
            let b = frames[(k + 1).min(frames.len() - 1)];
            [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f]
        })
        .collect()
}

/// How alike two channels' runs are (1 the same shape).
fn alike(a: impl Iterator<Item = f32> + Clone, b: impl Iterator<Item = f32> + Clone) -> f32 {
    let ab: f32 = a.clone().zip(b.clone()).map(|(x, y)| x * y).sum();
    let aa: f32 = a.map(|x| x * x).sum();
    let bb: f32 = b.map(|x| x * x).sum();
    ab / (aa * bb).sqrt().max(1e-12)
}

/// The source through what Bevy plays a source with (rodio 0.20.1: a
/// sink made and the source appended to it at once, the sink's queue in
/// an output's mixer at 48 kHz, two channels), a sound mixed in a tenth
/// of a second later comes out whole from its first frame, each channel
/// its own. A sound appended to a sink of its own came out with its
/// first stretch read as one channel at 48 kHz (`sound.md`, "Played").
#[test]
fn a_sound_comes_out_whole_from_its_start() {
    let mixed = Mixed::default();
    let (controller, mut mixer) = rodio::dynamic_mixer::mixer::<f32>(2, 48_000);
    let (sink, queue) = rodio::Sink::new_idle();
    controller.add(queue);
    sink.append(mixed.decoder());
    let mut out: Vec<f32> = (0..9_600).map(|_| mixer.next().unwrap_or(0.0)).collect();
    assert!(out.iter().all(|s| s.abs() < 1e-6), "quiet before the sound");
    mixed.add(Voice::whole(&samples(&clip()), 1.0));
    out.extend((0..19_200).map(|_| mixer.next().unwrap_or(0.0)));
    let frames: Vec<[f32; 2]> = out.chunks_exact(2).map(|f| [f[0], f[1]]).collect();
    let start = frames.iter().position(|f| f[0].abs() > 1e-4 || f[1].abs() > 1e-4).expect("the sound comes out");
    let expected = at_48k(&clip());
    // Its first 10 ms (480 frames) and its next 40, each channel against
    // the clip's own, at the best shift of a frame or two.
    for (from, to) in [(0, 480), (480, 2400)] {
        for channel in 0..2 {
            let best = (0..4)
                .map(|shift| {
                    let got = frames[start + from..start + to].iter().map(move |f| f[channel]);
                    let want = expected[(from + shift).min(expected.len())..(to + shift).min(expected.len())].iter().map(move |f| f[channel]);
                    alike(got, want)
                })
                .fold(-1.0f32, f32::max);
            assert!(best > 0.99, "channel {channel}, frames {from} to {to}: {best}");
        }
    }
    // The two channels are not one another.
    let left = frames[start..start + 480].iter().map(|f| f[0]);
    let right = frames[start..start + 480].iter().map(|f| f[1]);
    assert!(alike(left, right).abs() < 0.5);
}

/// A stereo clip of float frames, as the install keeps clips: 16 bits.
fn samples(frames: &[[f32; 2]]) -> Samples {
    let data = frames.iter().flat_map(|f| f.map(|s| (s * 32767.0) as i16)).collect();
    Samples { data: Arc::new(data), channels: 2, rate: RATE }
}

/// The first block the source gives with these sounds in it.
fn first_block(voices: Vec<Voice>) -> Vec<f32> {
    let mixed = Mixed::default();
    for voice in voices {
        mixed.add(voice);
    }
    let mut mixing = mixed.decoder();
    (0..BLOCK * 2).map(|_| mixing.next().unwrap()).collect()
}

/// A mono clip plays on both channels at 1/√2 of its level, a stereo one
/// as it is.
#[test]
fn a_mono_clip_is_played_at_the_middle_s_equal_power() {
    let mono = Samples::of(&trp_data::audio::sound::Clip { name: "m".into(), channels: 1, frequency: RATE, samples: vec![16384, -16384] });
    let out = first_block(vec![Voice::whole(&mono, 1.0)]);
    assert!((out[0] - 0.5 * MIDDLE).abs() < 1e-6 && out[0] == out[1]);
    assert!((out[2] + 0.5 * MIDDLE).abs() < 1e-6 && out[4] == 0.0);
    let stereo = Samples::of(&trp_data::audio::sound::Clip { name: "s".into(), channels: 2, frequency: RATE, samples: vec![16384, -8192] });
    assert_eq!(&first_block(vec![Voice::whole(&stereo, 1.0)])[..4], &[0.5, -0.25, 0.0, 0.0]);
}

/// A clip at another rate is read at its own; a pitch reads it faster;
/// a point's stereo clip is taken as one channel, each side at its gain.
#[test]
fn rates_pitches_and_a_point_s_gains() {
    let ramp: Vec<i16> = (0..400).map(|i| (i * 64) as i16).collect();
    let fast = Samples::of(&trp_data::audio::sound::Clip { name: "f".into(), channels: 1, frequency: 2 * RATE, samples: ramp.clone() });
    let out = first_block(vec![Voice::whole(&fast, 1.0 / MIDDLE)]);
    assert!((out[2] - 128.0 / 32768.0).abs() < 1e-6, "{}", out[2]);
    let pitch: Pitch = Arc::new(std::sync::atomic::AtomicU32::new(1.5f32.to_bits()));
    let plain = Samples::of(&trp_data::audio::sound::Clip { name: "p".into(), channels: 1, frequency: RATE, samples: ramp });
    let out = first_block(vec![Voice::placed(&plain, [1.0, 0.5], pitch)]);
    // The second frame is read half way between the clip's first two
    // past it: 1.5 frames in.
    assert!((out[2] - 96.0 / 32768.0).abs() < 1e-6 && (out[3] - 48.0 / 32768.0).abs() < 1e-6, "{} {}", out[2], out[3]);
    let pair = Samples::of(&trp_data::audio::sound::Clip { name: "s".into(), channels: 2, frequency: RATE, samples: vec![16384, 0] });
    let one: Pitch = Arc::new(std::sync::atomic::AtomicU32::new(1.0f32.to_bits()));
    let out = first_block(vec![Voice::placed(&pair, [1.0, 0.0], one)]);
    assert_eq!(&out[..2], &[0.25, 0.0]);
}

/// A sound in a group follows the group's level as it plays, and its
/// own level besides (decibels summed): 6.02 dB down halves it, and the
/// interface's sounds, in no group, are untouched.
#[test]
fn a_sound_follows_its_group_s_and_its_own_level() {
    let flat = Samples::of(&trp_data::audio::sound::Clip { name: "f".into(), channels: 2, frequency: RATE, samples: vec![16384; 2 * BLOCK * 4] });
    let mixed = Mixed::default();
    let own: Level = Default::default();
    set_level(&mixed.level(Group::ClientPlayer), -20.0 * 2f32.log10());
    set_level(&own, -20.0 * 2f32.log10());
    mixed.add_to(Group::ClientPlayer, Voice::whole(&flat, 1.0).following_level(own));
    mixed.add_to(Group::Guns, Voice::whole(&flat, 1.0));
    mixed.add(Voice::whole(&flat, 1.0));
    let mut mixing = mixed.decoder();
    let first: Vec<f32> = (0..2).map(|_| mixing.next().unwrap()).collect();
    // A quarter, plus two whole.
    assert!((first[0] - 0.5 * (0.25 + 1.0 + 1.0)).abs() < 1e-5, "{first:?}");
}

/// A sound told its gains anew begins at them as they stood, then goes to
/// what it is told by a share each block and a straight line within it:
/// no frame's level jumps, each side goes its own way, and it gets there.
/// A sound not told keeps its own from the first frame.
#[test]
fn a_sound_told_its_gains_anew_glides_to_them() {
    let flat = Samples::of(&trp_data::audio::sound::Clip { name: "f".into(), channels: 1, frequency: RATE, samples: vec![16384; BLOCK * 200] });
    let mixed = Mixed::default();
    let told = gains_of([1.0, 0.0]);
    let one: Pitch = Arc::new(std::sync::atomic::AtomicU32::new(1.0f32.to_bits()));
    mixed.add(Voice::placed(&flat, [0.3, 0.3], one).following_gains(told.clone()));
    let mut mixing = mixed.decoder();
    let mut block = || -> Vec<[f32; 2]> { (0..BLOCK).map(|_| [mixing.next().unwrap(), mixing.next().unwrap()]).collect() };
    // As told when it was made, not as it was placed.
    let first = block();
    assert!(first.iter().all(|f| *f == [0.5, 0.0]), "{:?}", first[0]);
    // Told the other side: the next block goes an eighth of the way, its
    // last frame there, each frame a 64th of that from the one before.
    set_gains(&told, [0.0, 1.0]);
    let second = block();
    assert!((second[BLOCK - 1][0] - 0.5 * 0.875).abs() < 1e-6 && (second[BLOCK - 1][1] - 0.5 * 0.125).abs() < 1e-6, "{:?}", second[BLOCK - 1]);
    let mut last = first[BLOCK - 1];
    let mut steps = second;
    for _ in 0..150 {
        steps.extend(block());
    }
    for frame in &steps {
        assert!(frame[0] <= last[0] && frame[1] >= last[1], "each side goes one way: {last:?} then {frame:?}");
        assert!((frame[0] - last[0]).abs() <= 0.5 * GLIDE / BLOCK as f32 + 1e-6 && (frame[1] - last[1]).abs() <= 0.5 * GLIDE / BLOCK as f32 + 1e-6, "no jump: {last:?} then {frame:?}");
        last = *frame;
    }
    assert_eq!(last, [0.0, 0.5], "it gets there");
    // One not told plays at its own from its first frame.
    let pitch: Pitch = Arc::new(std::sync::atomic::AtomicU32::new(1.0f32.to_bits()));
    assert_eq!(&first_block(vec![Voice::placed(&flat, [0.3, 0.6], pitch)])[..2], &[0.5 * 0.3, 0.5 * 0.6]);
}

/// The two sides of a point's pan: equal ahead and where the listener
/// stands, wholly one side abeam, the power the same all round.
#[test]
fn a_point_s_pan_is_of_equal_power() {
    let right = Vec3::X;
    for to in [Vec3::ZERO, Vec3::new(0.0, 0.0, -5.0), Vec3::new(0.0, 2.0, 5.0)] {
        let both = pan(to, right);
        assert!((both[0] - MIDDLE).abs() < 1e-6 && (both[1] - MIDDLE).abs() < 1e-6, "{to}: {both:?}");
    }
    let (to_right, to_left) = (pan(Vec3::new(3.0, 0.0, 0.0), right), pan(Vec3::new(-3.0, 0.0, 0.0), right));
    assert!(to_right[0].abs() < 1e-6 && (to_right[1] - 1.0).abs() < 1e-6 && (to_left[0] - 1.0).abs() < 1e-6 && to_left[1].abs() < 1e-6);
    let slant = pan(Vec3::new(2.0, 1.0, -4.0), right);
    assert!(slant[1] > slant[0] && (slant[0] * slant[0] + slant[1] * slant[1] - 1.0).abs() < 1e-6);
}

/// A loop scheduled at a frame of the mix plays from there round and
/// round until its cut, and the sound scheduled at the cut's frame
/// takes over on that very frame (a weapon's loop and its tail).
#[test]
fn a_loop_is_cut_on_the_frame_its_tail_begins() {
    // A loop of four frames counting up, and a tail of one level.
    let loop_clip = Samples::of(&trp_data::audio::sound::Clip { name: "loop".into(), channels: 2, frequency: RATE, samples: vec![1000, 1000, 2000, 2000, 3000, 3000, 4000, 4000] });
    let tail_clip = Samples::of(&trp_data::audio::sound::Clip { name: "tail".into(), channels: 2, frequency: RATE, samples: vec![-8000; 2 * 200] });
    let mixed = Mixed::default();
    let (cut, never) = (no_cut(), no_cut());
    cut.store(70, std::sync::atomic::Ordering::Release);
    mixed.add(Voice::whole(&loop_clip, 1.0).at_frame(10, true, cut));
    mixed.add(Voice::whole(&tail_clip, 1.0).at_frame(70, false, never));
    let mut mixing = mixed.decoder();
    let left: Vec<f32> = (0..BLOCK * 2 * 2).map(|_| mixing.next().unwrap()).step_by(2).collect();
    let level = |frame: usize| (left[frame] * 32768.0).round() as i32;
    assert_eq!(level(9), 0, "nothing before the loop's frame");
    assert_eq!((level(10), level(11), level(13), level(14)), (1000, 2000, 4000, 1000), "the loop from its first frame, round again after four");
    assert_eq!((level(69), level(70)), (4000, -8000), "the loop's last frame, then the tail's first");
    assert_eq!(mixed.clock(), 2 * BLOCK as u64);
}
