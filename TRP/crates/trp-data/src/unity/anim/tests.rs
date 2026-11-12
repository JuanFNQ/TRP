//! Tests of the animation clips: path hashes, rest rotations and the sampling of their curves.

use super::*;

#[test]
fn path_hash_is_crc32() {
    assert_eq!(path_hash(""), 0);
    assert_eq!(path_hash("123456789"), 0xcbf4_3926);
}

/// Angles from the first frame of the AKM idle clip for the left hand
/// marker, against the rotation the prefab stores for the same bone.
#[test]
fn euler_order_reproduces_a_stored_rest_rotation() {
    let q = euler_to_quat([-76.80162, 17.92335, 125.08804]);
    let rest = [-0.391, -0.488, 0.732, 0.271];
    for (a, b) in q.iter().zip(rest) {
        assert!((a - b).abs() < 2e-3, "{q:?} != {rest:?}");
    }
}

/// A clip of curves kept as samples and nothing else, ten a second.
fn sampled(curves: usize, samples: Vec<f32>) -> Clip {
    let frames = samples.len() / curves;
    Clip {
        events: Vec::new(),
        name: String::new(),
        start_time: 0.0,
        stop_time: (frames - 1) as f32 / 10.0,
        looping: false,
        tracks: Vec::new(),
        floats: Vec::new(),
        reference_pose: Vec::new(),
        root_stays_level: false,
        root_stays_upright: false,
        streamed: Vec::new(),
        streamed_curves: 0,
        dense_frames: frames,
        dense_curves: curves,
        dense_rate: 10.0,
        dense_begin: 0.0,
        dense: samples,
        constant: Vec::new(),
        by_curve: std::sync::OnceLock::new(),
        others: Vec::new(),
    }
}

/// The curve for drawing goes through every sample, keeps a straight
/// line straight, follows a curved one between its samples where
/// straight lines cut across it, has no corner at a sample, and does
/// not overshoot a jump.
#[test]
fn the_curve_for_drawing_has_no_corners_and_overshoots_nothing() {
    // Three curves: a straight line, a parabola, and a jump.
    let line = |t: f32| 3.0 * t - 1.0;
    let parabola = |t: f32| 40.0 * t * t;
    let jump = |frame: usize| if frame < 5 { 0.0 } else { 1.0 };
    let clip = sampled(3, (0..10).flat_map(|frame| [line(frame as f32 / 10.0), parabola(frame as f32 / 10.0), jump(frame)]).collect());
    let (mut straight, mut smooth) = (Vec::new(), Vec::new());
    // At the samples the two agree.
    for frame in 0..10 {
        clip.sample(frame as f32 / 10.0, &mut straight);
        clip.sample_smooth(frame as f32 / 10.0, &mut smooth);
        for c in 0..3 {
            assert!((straight[c] - smooth[c]).abs() < 1e-5, "sample {frame}, curve {c}: {} and {}", straight[c], smooth[c]);
        }
    }
    let (mut worst_straight, mut worst_smooth) = (0.0f32, 0.0f32);
    let mut last: Option<Vec<f32>> = None;
    let mut slopes: Vec<f32> = Vec::new();
    for step in 0..=900 {
        let t = step as f32 / 1000.0;
        clip.sample(t, &mut straight);
        clip.sample_smooth(t, &mut smooth);
        assert!((smooth[0] - line(t)).abs() < 1e-4, "the line at {t}: {}", smooth[0]);
        // Away from the clip's two ends, where there is one step only.
        if (0.1..=0.8).contains(&t) {
            worst_straight = worst_straight.max((straight[1] - parabola(t)).abs());
            worst_smooth = worst_smooth.max((smooth[1] - parabola(t)).abs());
        }
        assert!((-1e-6..=1.0 + 1e-6).contains(&smooth[2]), "the jump is overshot at {t}: {}", smooth[2]);
        if let Some(last) = &last {
            slopes.push((smooth[1] - last[1]) * 1000.0);
        }
        last = Some(smooth.clone());
    }
    // Straight lines cut across the parabola by a tenth (40 times a
    // step squared over eight); the curve follows it.
    assert!((worst_straight - 0.1).abs() < 1e-3, "{worst_straight}");
    assert!(worst_smooth < 1e-3, "{worst_smooth}");
    // No corner: the slope from one thousandth to the next never
    // changes by more than the parabola's own bend allows (80 a
    // second each second), where straight lines change it by 8 at
    // every sample.
    let sharpest = slopes.windows(2).map(|pair| (pair[1] - pair[0]).abs()).fold(0.0f32, f32::max);
    assert!(sharpest < 0.5, "{sharpest}");
}

/// A clip of one curve kept as cubics, each key a time, a value and
/// the cubic's two upper terms (none for a straight line to the next
/// key); its samples would be ten a second.
fn keyed(keys: &[(f32, f32, [f32; 2])]) -> Clip {
    let mut clip = sampled(1, vec![0.0, 0.0]);
    (clip.dense_frames, clip.dense_curves, clip.dense) = (0, 0, Vec::new());
    clip.stop_time = keys.last().unwrap().0;
    clip.streamed_curves = 1;
    for (index, (time, value, bend)) in keys.iter().enumerate() {
        let line = keys.get(index + 1).map_or(0.0, |next| (next.1 - value) / (next.0 - time));
        clip.streamed.push(StreamedFrame { time: *time, keys: vec![StreamedKey { curve: 0, coeff: [bend[0], bend[1], line, *value] }] });
    }
    clip.streamed.push(StreamedFrame { time: f32::INFINITY, keys: Vec::new() });
    clip
}

/// A curve kept as cubics with a key at every sample and straight
/// lines between is taken on the curve for drawing as samples are; a
/// straight line across many samples, and a span that is a curve of
/// its maker's, are left as they are.
/// One curve sampled alone is what the whole sample has for it, to the
/// bit, streamed, dense and constant.
#[test]
fn a_curve_sampled_alone_is_the_whole_sample_s() {
    let clip = keyed(&[(0.0, 1.0, [0.0, 0.5]), (0.033, 2.0, [3.0, -1.0]), (0.45, 0.25, [-2.0, 0.0]), (1.0, 3.5, [0.5, 0.5])]);
    let mut whole = Vec::new();
    for step in -10..=1200 {
        let t = step as f32 / 1000.0;
        clip.sample(t, &mut whole);
        for (curve, value) in whole.iter().enumerate() {
            assert_eq!(clip.sample_curve(t, curve).to_bits(), value.to_bits(), "curve {curve} at {t}");
        }
    }
    let mut dense = sampled(2, vec![0.0, 1.0, 2.0, 3.0, 4.0, 8.0]);
    dense.constant = vec![7.0];
    for step in -10..=400 {
        let t = step as f32 / 1000.0;
        dense.sample(t, &mut whole);
        for (curve, value) in whole.iter().enumerate() {
            assert_eq!(dense.sample_curve(t, curve).to_bits(), value.to_bits(), "dense curve {curve} at {t}");
        }
    }
}

/// A curve's keys found by its own list (the frames in time order)
/// give every sample the walk of the frames gives, to the bit, both
/// ways of sampling, before, between, on and after the keys.
#[test]
fn keys_found_by_curve_are_the_walk_s() {
    let clip = keyed(&[(0.0, 1.0, [0.0, 0.5]), (0.033, 2.0, [3.0, -1.0]), (0.1, -4.0, [0.0, 0.0]), (0.1, 7.0, [1.0, 1.0]), (0.45, 0.25, [-2.0, 0.0]), (0.5, 3.0, [0.0, 0.0]), (1.0, 3.5, [0.5, 0.5])]);
    let walked = clip.clone();
    walked.by_curve.set(None).expect("not yet made");
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for step in -10..=1200 {
        let t = step as f32 / 1000.0;
        clip.sample(t, &mut a);
        walked.sample(t, &mut b);
        assert_eq!(a.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), b.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), "sample at {t}");
        clip.sample_smooth(t, &mut a);
        walked.sample_smooth(t, &mut b);
        assert_eq!(a.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), b.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), "smooth sample at {t}");
    }
    assert!(clip.by_curve.get().is_some_and(Option::is_some), "indexed");
}

#[test]
fn keys_at_every_sample_are_drawn_as_samples_are() {
    let parabola = |t: f32| 40.0 * t * t;
    let clip = keyed(&(0..10).map(|key| (key as f32 / 10.0, parabola(key as f32 / 10.0), [0.0, 0.0])).collect::<Vec<_>>());
    let (mut straight, mut smooth) = (Vec::new(), Vec::new());
    let (mut worst_straight, mut worst_smooth) = (0.0f32, 0.0f32);
    for step in 100..=800 {
        let t = step as f32 / 1000.0;
        clip.sample(t, &mut straight);
        clip.sample_smooth(t, &mut smooth);
        worst_straight = worst_straight.max((straight[0] - parabola(t)).abs());
        worst_smooth = worst_smooth.max((smooth[0] - parabola(t)).abs());
    }
    assert!((worst_straight - 0.1).abs() < 1e-3 && worst_smooth < 1e-3, "{worst_straight}, {worst_smooth}");
    // A straight line a second long, between two rests: it stays one.
    let ramp = keyed(&[(0.0, 0.0, [0.0, 0.0]), (0.1, 0.0, [0.0, 0.0]), (1.1, 5.0, [0.0, 0.0]), (1.2, 5.0, [0.0, 0.0])]);
    // And a curved span a sample long: it stays the curve it is.
    let curved = keyed(&[(0.0, 0.0, [0.0, 0.0]), (0.1, 1.0, [300.0, -40.0]), (0.2, 2.0, [0.0, 0.0]), (0.3, 3.0, [0.0, 0.0])]);
    for (clip, times) in [(&ramp, [0.15, 0.6, 1.05]), (&curved, [0.12, 0.15, 0.18])] {
        for t in times {
            clip.sample(t, &mut straight);
            clip.sample_smooth(t, &mut smooth);
            assert_eq!(straight, smooth, "at {t}");
        }
    }
}

/// A clip's part (`part`): every curve it keeps samples to the bit as
/// in the whole clip, on straight lines and smooth, before, between and
/// past its keys and samples, its reference, tracks and animator curves
/// renumbered; with the frames in time order, and out of it (every frame
/// kept, the walk stopping where the whole clip's does).
#[test]
fn a_part_samples_as_the_whole_clip_s() {
    // Three curves kept as cubics with keys at uneven times (one with a
    // key on every sample), two as samples, one figure.
    let mut whole = sampled(2, (0..12).flat_map(|frame| [(frame as f32 * 0.7).sin(), (frame * frame) as f32 * 0.01]).collect());
    whole.streamed_curves = 3;
    let keys = [(0.0, [0.0, 0.5, 1.0, 1.0]), (0.033, [3.0, -1.0, 0.2, 2.0]), (0.1, [0.0, 0.0, 30.0, -4.0]), (0.2, [0.0, 0.0, -5.0, 7.0]), (0.45, [-2.0, 0.0, 1.0, 0.25]), (0.9, [0.5, 0.5, 0.0, 3.5])];
    for (n, (time, coeff)) in keys.iter().enumerate() {
        let mut frame = StreamedFrame { time: *time, keys: vec![StreamedKey { curve: n % 3, coeff: *coeff }] };
        if n % 2 == 0 {
            frame.keys.push(StreamedKey { curve: 2, coeff: [0.0, 0.0, 10.0, n as f32 * 0.1] });
        }
        whole.streamed.push(frame);
    }
    whole.streamed.push(StreamedFrame { time: f32::INFINITY, keys: Vec::new() });
    whole.constant = vec![9.5];
    whole.stop_time = 1.1;
    let count = whole.curve_count();
    assert_eq!(count, 6);
    whole.reference_pose = (0..count).map(|c| c as f32 * 1.5 - 2.0).collect();
    whole.tracks = vec![Track { path: 7, property: Property::Position, curve: 0, custom_type: 0 }, Track { path: 8, property: Property::Position, curve: 3, custom_type: 0 }];
    whole.floats = vec![FloatCurve { name: 1, curve: 5, custom_type: 0 }];
    let mut shuffled = whole.clone();
    shuffled.streamed.swap(1, 4);
    for clip in [&whole, &shuffled] {
        for wanted in [[true, false, true, false, true, true], [false, true, false, true, false, false], [true; 6]] {
            let part = clip.part(&wanted);
            let kept: Vec<usize> = (0..count).filter(|c| wanted[*c]).collect();
            assert_eq!(part.curve_count(), kept.len());
            assert_eq!(part.reference_pose, kept.iter().map(|c| clip.reference_pose[*c]).collect::<Vec<_>>());
            // In order, the frames left without a key dropped; out of it, every one kept.
            if std::ptr::eq(clip, &whole) {
                assert!(part.streamed.iter().all(|frame| !frame.keys.is_empty()));
            } else {
                assert_eq!(part.streamed.len(), clip.streamed.len());
            }
            let (mut a, mut b) = (Vec::new(), Vec::new());
            for step in -10..=1300 {
                let t = step as f32 / 1000.0;
                for smooth in [false, true] {
                    if smooth {
                        clip.sample_smooth(t, &mut a);
                        part.sample_smooth(t, &mut b);
                    } else {
                        clip.sample(t, &mut a);
                        part.sample(t, &mut b);
                    }
                    for (new, old) in kept.iter().enumerate() {
                        assert_eq!(a[*old].to_bits(), b[new].to_bits(), "curve {old} at {t} (smooth {smooth}, wanted {wanted:?})");
                        assert_eq!(clip.sample_curve(t, *old).to_bits(), part.sample_curve(t, new).to_bits(), "curve {old} alone at {t}");
                    }
                }
            }
            for track in &part.tracks {
                assert!(clip.tracks.iter().any(|t| t.path == track.path && kept[track.curve] == t.curve));
            }
            assert_eq!(part.floats.len(), usize::from(wanted[5]));
        }
    }
}

#[test]
fn dense_curves_interpolate_and_constants_follow() {
    let clip = Clip {
        events: Vec::new(),
        name: String::new(),
        start_time: 0.0,
        stop_time: 1.0,
        looping: false,
        tracks: Vec::new(),
        floats: Vec::new(),
        reference_pose: Vec::new(),
        root_stays_level: false,
        root_stays_upright: false,
        streamed: vec![
            StreamedFrame {
                time: f32::NEG_INFINITY,
                keys: vec![StreamedKey { curve: 0, coeff: [0.0, 0.0, 0.0, 5.0] }],
            },
            // From 0.5 s the curve is 1 + 2 * dt.
            StreamedFrame { time: 0.5, keys: vec![StreamedKey { curve: 0, coeff: [0.0, 0.0, 2.0, 1.0] }] },
            StreamedFrame { time: f32::INFINITY, keys: Vec::new() },
        ],
        streamed_curves: 1,
        dense_frames: 3,
        dense_curves: 1,
        dense_rate: 2.0,
        dense_begin: 0.0,
        dense: vec![0.0, 10.0, 20.0],
        constant: vec![9.0],
        by_curve: std::sync::OnceLock::new(),
        others: Vec::new(),
    };
    let mut v = Vec::new();
    clip.sample(0.25, &mut v);
    assert_eq!(v, [5.0, 5.0, 9.0]);
    clip.sample(0.75, &mut v);
    assert_eq!(v, [1.5, 15.0, 9.0]);
    clip.sample(5.0, &mut v);
    assert_eq!(v, [2.0, 20.0, 9.0]);

    // Cut down to the first and last curves, the same values come out
    // under their new numbers.
    let mut whole = clip.clone();
    whole.floats = (0..3).map(|curve| FloatCurve { name: curve as u32, curve, custom_type: 0 }).collect();
    whole.reference_pose = vec![1.0, 2.0, 3.0];
    let cut = whole.reduced(|_| false, |f| f.name != 1);
    assert_eq!(cut.curve_count(), 2);
    assert_eq!(cut.floats.iter().map(|f| (f.name, f.curve)).collect::<Vec<_>>(), [(0, 0), (2, 1)]);
    assert_eq!(cut.reference_pose, [1.0, 3.0]);
    cut.sample(0.75, &mut v);
    assert_eq!(v, [1.5, 9.0]);
}
