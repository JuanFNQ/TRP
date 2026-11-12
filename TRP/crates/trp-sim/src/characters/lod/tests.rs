//! Tests of the levels of detail: the engine's pick, TRP's own rule and the props' levels.
use super::*;

/// The install's groups (`probe dump`): a body's and feet's (size 2; 0.6, 0.015), a
/// head's (0.6, 0.035), the AKM's container (size 0.889; 0.11, 0.01).
const BODY: [f32; 2] = [0.6, 0.015];
const HEAD: [f32; 2] = [0.6, 0.035];
const AKM: [f32; 2] = [0.11, 0.01];

/// The size on screen (`pixels_tall`) is the size over the pixel, and a
/// whole thing goes under a pixel of it; a character's posing for
/// drawing by it: a 2 m figure at 1440 lines and 50 degrees is 32 px
/// at 96.5 m, held below, and goes past 3,088 m; the figures scale with
/// the tolerance.
#[test]
fn a_characters_posing_goes_by_its_size_on_screen() {
    let clip_y = 1.0 / (25.0f32).to_radians().tan();
    let tall = |d: f32| pixels_tall(2.0, d, 1440.0, clip_y, false);
    assert!((tall(10.0) - 2.0 / pixel(10.0, 1440.0, clip_y, false)).abs() < 1e-3);
    assert!((tall(96.5) - 32.0).abs() < 0.05, "{}", tall(96.5));
    for d in [1.0, 50.0, 3087.0, 3089.0] {
        assert_eq!(tall(d) < CULL_PIXELS, culled_whole(2.0, pixel(d, 1440.0, clip_y, false)), "{d}");
    }
    assert_eq!(posing_drawn(Some(tall(90.0)), 1.0), PosingDrawn::EveryFrame);
    assert_eq!(posing_drawn(Some(tall(100.0)), 1.0), PosingDrawn::Held(4));
    assert_eq!(posing_drawn(Some(tall(3000.0)), 1.0), PosingDrawn::Held(4));
    assert_eq!(posing_drawn(Some(tall(3100.0)), 1.0), PosingDrawn::Not);
    assert_eq!(posing_drawn(None, 1.0), PosingDrawn::Not);
    // At a bias of 4 (half a pixel): every frame to twice as far, held
    // for two frames; at 2.5, held for three.
    let half = tolerance(4.0);
    assert_eq!(posing_drawn(Some(tall(190.0)), half), PosingDrawn::EveryFrame);
    assert_eq!(posing_drawn(Some(tall(200.0)), half), PosingDrawn::Held(2));
    assert_eq!(posing_drawn(Some(tall(130.0)), tolerance(2.5)), PosingDrawn::Held(3));
}

#[test]
fn at_the_installs_bias_and_field_of_view() {
    // Bias 2 (`Graphics.ini`), 74 degrees (`Game.ini`).
    let (bias, fov) = (2.0, 74.0);
    assert!((reach(2.0, 1.0, BODY[0], fov, bias) - 4.42).abs() < 0.01, "{}", reach(2.0, 1.0, BODY[0], fov, bias));
    assert!((reach(2.0, 1.0, BODY[1], fov, bias) - 176.9).abs() < 0.1);
    assert!((reach(2.0, 1.0, HEAD[1], fov, bias) - 75.8).abs() < 0.1);
    assert!((reach(0.889, 1.0, AKM[0], fov, bias) - 10.72).abs() < 0.01);
    assert!((reach(0.889, 1.0, AKM[1], fov, bias) - 117.9).abs() < 0.1);
    assert_eq!(pick(2.0, 1.0, 4.4, fov, bias, &BODY), Some(0));
    assert_eq!(pick(2.0, 1.0, 4.45, fov, bias, &BODY), Some(1));
    assert_eq!(pick(2.0, 1.0, 176.0, fov, bias, &BODY), Some(1));
    assert_eq!(pick(2.0, 1.0, 177.0, fov, bias, &BODY), None);
    assert_eq!(pick(2.0, 1.0, 75.0, fov, bias, &HEAD), Some(1));
    assert_eq!(pick(2.0, 1.0, 76.0, fov, bias, &HEAD), None);
    assert_eq!(pick(0.889, 1.0, 10.7, fov, bias, &AKM), Some(0));
    assert_eq!(pick(0.889, 1.0, 10.75, fov, bias, &AKM), Some(1));
    assert_eq!(pick(0.889, 1.0, 118.0, fov, bias, &AKM), None);
    // The lanes start: the walker 5.4 to 6.0 m, the stander 6.25 m.
    assert_eq!(pick(2.0, 1.0, 5.4, fov, bias, &BODY), Some(1));
}

#[test]
fn at_trps_own_field_of_view() {
    // 50 degrees, TRP's options' default.
    assert!((reach(2.0, 1.0, BODY[0], 50.0, 2.0) - 7.15).abs() < 0.01, "{}", reach(2.0, 1.0, BODY[0], 50.0, 2.0));
    assert_eq!(pick(2.0, 1.0, 6.25, 50.0, 2.0, &BODY), Some(0));
    assert_eq!(pick(2.0, 1.0, 7.2, 50.0, 2.0, &BODY), Some(1));
}

#[test]
fn the_documented_rule_picks_as_the_engine() {
    for distance in [0.5, 3.5, 4.0, 4.43, 6.0, 25.0, 33.6, 80.0, 180.0, 300.0] {
        for fov in [50.0, 74.0, 75.0] {
            for bias in [2.0, 2.5, 3.0, 3.5, 4.0] {
                let relative = relative_height(2.0, 1.0, distance, fov, bias);
                assert_eq!(level(&BODY, relative), pick(2.0, 1.0, distance, fov, bias, &BODY), "{distance} m, {fov} degrees, bias {bias}");
            }
        }
    }
    // At the camera itself, the fullest.
    assert_eq!(level(&BODY, relative_height(2.0, 1.0, 0.0, 74.0, 2.0)), Some(0));
    assert_eq!(pick(2.0, 1.0, 0.0, 74.0, 2.0, &BODY), Some(0));
}

#[test]
fn the_main_cameras_controller_picks_as_at_the_options_view() {
    // Aimed, 15 degrees narrower: the bias scaled down by as much as the
    // view, so the pick stays the options' view's.
    let bias = controlled_bias(2.0, 59.0, 74.0);
    for distance in [4.0, 4.43, 4.5, 75.0, 76.0, 176.0, 177.0] {
        assert_eq!(pick(2.0, 1.0, distance, 59.0, bias, &BODY), pick(2.0, 1.0, distance, 74.0, 2.0, &BODY), "{distance} m");
    }
    assert_eq!(controlled_bias(2.0, 74.0, 74.0), 2.0);
}

/// The range's loose things and props (`probe dump`, `probe popper`,
/// `probe sceneprops`; `the_loose_things_and_props_levels_
/// are_read`): the AK magazine lying on the table (size 0.2013; 0.0971,
/// 0.01), a 7.62x39 round or its spent case (0.0556; 0.2005, 0.01), the
/// yard's key (0.5268; 0.0453 alone), a popper (1.2005; 0.19, 0.03075)
/// and the gun bench's vise (0.6737; 0.3988, 0.0408). Where each is
/// left, and a loose thing's pick at a distance.
#[test]
fn the_loose_things_and_props_levels() {
    const MAGAZINE: (f32, [f32; 2]) = (0.2013, [0.0971, 0.01]);
    const ROUND: (f32, [f32; 2]) = (0.0556, [0.2005, 0.01]);
    const KEY: (f32, [f32; 1]) = (0.5268, [0.0453]);
    const POPPER: (f32, [f32; 2]) = (1.2005, [0.19, 0.03075]);
    const VISE: (f32, [f32; 2]) = (0.6737, [0.3988, 0.0408]);
    let (bias, fov) = (2.0, 74.0);
    let left = |size: f32, height: f32, fov: f32| reach(size, 1.0, height, fov, bias);
    // At the install's view: the magazine coarser from 2.75 m, gone at
    // 26.7 m (the AKM's own magazine's figure); a round coarser
    // from 0.37 m and gone at 7.4 m; the key gone at 15.5 m; a popper
    // coarser from 8.4 m and gone at 51.8 m; the vise gone at 21.9 m.
    for (got, want) in [
        (left(MAGAZINE.0, MAGAZINE.1[0], fov), 2.75),
        (left(MAGAZINE.0, MAGAZINE.1[1], fov), 26.71),
        (left(ROUND.0, ROUND.1[0], fov), 0.368),
        (left(ROUND.0, ROUND.1[1], fov), 7.38),
        (left(KEY.0, KEY.1[0], fov), 15.45),
        (left(POPPER.0, POPPER.1[0], fov), 8.385),
        (left(POPPER.0, POPPER.1[1], fov), 51.80),
        (left(VISE.0, VISE.1[1], fov), 21.93),
    ] {
        assert!((got - want).abs() < 0.01 * want.max(1.0), "{got} against {want}");
    }
    // At TRP's own 50 degrees: the round gone at 11.9 m, a popper at
    // 83.6 m (so the 100, 200 and 300 m rows draw nothing by this rule).
    assert!((left(ROUND.0, ROUND.1[1], 50.0) - 11.92).abs() < 0.02);
    assert!((left(POPPER.0, POPPER.1[1], 50.0) - 83.71).abs() < 0.1);
    // A magazine on the floor at 2, 3, 20 and 30 m, at the install's view.
    let magazine = |distance: f32| pick(MAGAZINE.0, 1.0, distance, fov, bias, &MAGAZINE.1);
    assert_eq!([magazine(2.0), magazine(3.0), magazine(20.0), magazine(30.0)], [Some(0), Some(1), Some(1), None]);
    // A round at the eye's 0.3 m, at 1 m, and past 7.4 m.
    let round = |distance: f32| pick(ROUND.0, 1.0, distance, fov, bias, &ROUND.1);
    assert_eq!([round(0.3), round(1.0), round(7.5)], [Some(0), Some(1), None]);
    // A group of one level: drawn to its reach, then nothing.
    assert_eq!([pick(KEY.0, 1.0, 15.0, fov, bias, &KEY.1), pick(KEY.0, 1.0, 16.0, fov, bias, &KEY.1)], [Some(0), None]);
    // The poppers' rows at 25, 50 and 100 m.
    let popper = |distance: f32| pick(POPPER.0, 1.0, distance, fov, bias, &POPPER.1);
    assert_eq!([popper(25.0), popper(50.0), popper(100.0)], [Some(1), Some(1), None]);
}

// TRP's own rule. The deviations are the install's as measured
// (`trps_measure_of_the_levels`): the bear body's level 1
// 13.6 mm from its fullest, the AKM's parts 2.3 to 17.2 mm, the
// magazine's 12.3 mm, a round's 1.0 mm, a popper's 5.6 mm (its caps
// not sampled).
const BODY_DEVIATION: [f32; 2] = [0.0, 0.013626];

/// The main view at 1440 pixels and TRP's 50 degrees.
fn at_1440(distance: f32) -> f32 {
    pixel(distance, 1440.0, 1.0 / half_tan(50.0), false)
}

#[test]
fn the_slider_maps_to_the_pixels_allowed() {
    // `OBJECT LOD QUALITY`'s five: one pixel at the game's own 2, half a
    // pixel at 4; each switch's distance scales with the setting as the
    // game's bias scales the game's.
    let allowed: Vec<f32> = [2.0, 2.5, 3.0, 3.5, 4.0].iter().map(|&bias| tolerance(bias)).collect();
    for (got, want) in allowed.iter().zip([1.0, 0.8, 0.6667, 0.5714, 0.5]) {
        assert!((got - want).abs() < 1e-3, "{allowed:?}");
    }
    let switch_at = |bias: f32| switches(&BODY_DEVIATION, 2, 1.0, tolerance(bias))[1] / at_1440(1.0);
    assert!((switch_at(4.0) / switch_at(2.0) - 2.0).abs() < 1e-4);
    let game = |bias: f32| reach(2.0, 1.0, BODY[0], 74.0, bias);
    assert!((game(4.0) / game(2.0) - switch_at(4.0) / switch_at(2.0)).abs() < 1e-4);
}

#[test]
fn a_level_switches_where_its_deviation_is_under_a_pixel() {
    // The pixel at 10 m in the main view: 2 × 10 / (1440 / tan 25°).
    assert!((at_1440(10.0) - 0.006477).abs() < 1e-5, "{}", at_1440(10.0));
    // An orthographic view's pixel (a shadow map's texel) is the same at
    // any distance: 40 m across on 2048 texels.
    assert!((pixel(3.0, 2048.0, 2.0 / 40.0, true) - 40.0 / 2048.0).abs() < 1e-7);
    assert_eq!(pixel(3.0, 2048.0, 2.0 / 40.0, true), pixel(300.0, 2048.0, 2.0 / 40.0, true));
    // The bear body: its coarser level from where its 13.6 mm is a
    // pixel, 21.0 m at 1440 pixels and 50 degrees (the game's: 4.42 m at
    // 74, where it is 2.9 pixels at 1440); 15.8 m at 1080.
    let from = switches(&BODY_DEVIATION, 2, 1.0, 1.0);
    let distance = |pixels: f32| from[1] * pixels / (2.0 * half_tan(50.0));
    assert!((distance(1440.0) - 21.04).abs() < 0.05, "{}", distance(1440.0));
    assert!((distance(1080.0) - 15.78).abs() < 0.05, "{}", distance(1080.0));
    assert_eq!(steady_level(&from, at_1440(20.0)), 0);
    assert_eq!(steady_level(&from, at_1440(22.0)), 1);
    let game_pixels = 0.013626 * 1440.0 / (2.0 * 4.42 * half_tan(74.0));
    assert!((game_pixels - 2.95).abs() < 0.05, "{game_pixels}");
    // Twice as far by the screen's own pixels with a narrower view: aimed
    // at 25 degrees the same body keeps its fullest to 44.3 m.
    let aimed = |distance: f32| pixel(distance, 1440.0, 1.0 / half_tan(25.0), false);
    assert_eq!(steady_level(&from, aimed(44.0)), 0);
    assert_eq!(steady_level(&from, aimed(44.5)), 1);
    // The scope's own view (1024 pixels, 7.375 degrees): to 108 m.
    let scope = |distance: f32| pixel(distance, 1024.0, 1.0 / half_tan(7.375), false);
    assert_eq!(steady_level(&from, scope(100.0)), 0);
    assert_eq!(steady_level(&from, scope(120.0)), 1);
    // An unmeasured group keeps its fullest at any distance.
    assert_eq!(steady_level(&switches(&[], 2, 1.0, 1.0), at_1440(1000.0)), 0);
}

#[test]
fn nothing_is_culled_alone() {
    // Past its last level a group keeps its coarsest: the head, the
    // body, a worn thing, at any distance.
    let from = switches(&[0.0, 0.0194], 2, 1.0, 1.0);
    for distance in [80.0, 177.0, 500.0, 2000.0] {
        assert_eq!(steady_level(&from, at_1440(distance)), 1);
        assert!(drawn(band(&from, 0b10), at_1440(distance)));
        assert!(!drawn(band(&from, 0b01), at_1440(distance)));
    }
    assert_eq!(band(&from, 0b10).1, f32::INFINITY);
    // A group of one level: drawn at any distance.
    assert!(drawn(band(&switches(&[0.0], 1, 1.0, 1.0), 0b1), at_1440(5000.0)));
    // A whole figure (its body's group, 2 m) goes only under a pixel:
    // past 3,088 m at 1440 and 50 degrees; a magazine (0.2013 m) past
    // 311 m, a round (0.0556 m) past 86 m, a popper (1.2005 m) past
    // 1,853 m (the range's 300 m row is 8 pixels).
    let gone = |size: f32| size / at_1440(1.0);
    for (size, want) in [(2.0, 3088.0), (0.2013, 310.8), (0.0556, 85.8), (1.2005, 1853.5)] {
        assert!((gone(size) - want).abs() < 0.002 * want, "{size}: {}", gone(size));
        assert!(!culled_whole(size, at_1440(want * 0.99)) && culled_whole(size, at_1440(want * 1.01)));
    }
    assert!((1.2005 / at_1440(300.0) - 6.18).abs() < 0.01);
}

#[test]
fn two_levels_fade_through_each_others_pixels() {
    let from = switches(&BODY_DEVIATION, 2, 1.0, 1.0);
    let (fine, coarse) = (band(&from, 0b01), band(&from, 0b10));
    let at = from[1];
    let mut faded = 0;
    for step in 0..=400 {
        let p = at * (0.9 + step as f32 * 0.001);
        let (a, b) = (dither(fine, p), dither(coarse, p));
        // Outside the fade one is whole and the other gone; inside, the
        // fine one's count out and the coarse one's in, at least
        // complementary (Bevy's test draws a pixel of the one where the
        // other's is thrown; the coarse one a step ahead covers more).
        if p < at * (1.0 - OVERLAP) {
            assert_eq!((a, b), (0, -16), "{p}");
        } else if p >= at * (1.0 + FADE) {
            assert_eq!(a, 16, "{p}");
            assert!(b == 0, "{p}");
        } else {
            assert!(b >= a - 16, "{p}: {a} {b}");
            faded += 1;
        }
        // Their places apart by 4% of the distance: still no pixel left.
        let (a_far, b_near) = (dither(fine, p), dither(coarse, p * 0.96));
        if a_far > 0 && a_far < 16 {
            assert!(b_near >= a_far - 16, "{p}: {a_far} {b_near}");
        }
        assert!(drawn(fine, p) || a == 16, "{p}");
        assert!(drawn(coarse, p) || b == -16, "{p}");
    }
    assert!(faded > 100);
    // The fade is a share of the distance: at 21 m it takes 3.2 m.
    assert!((21.04 * FADE - 3.16).abs() < 0.01);
}

#[test]
fn a_weapons_parts_switch_with_their_weapon() {
    // The AKM's groups (each part's own deviation, measured): every part
    // switches where the farthest-parting part does, 17.2 mm; never a
    // part on its own reach (the game's left the muzzle brake at 4.2 m).
    let parts = vec![vec![0.0, 0.017239], vec![0.0, 0.005481], vec![0.0, 0.003891], vec![0.0, 0.005931], vec![0.0, 0.012318], vec![]];
    let together = together(&parts);
    for (got, part) in together.iter().zip(&parts) {
        if part.is_empty() {
            assert!(got.is_empty());
        } else {
            assert_eq!(got, &vec![0.0, 0.017239]);
        }
    }
    // A group of three levels with one of two: the shorter taken at its
    // last for the third.
    assert_eq!(super::together(&[vec![0.0, 0.01, 0.03], vec![0.0, 0.02]]), vec![vec![0.0, 0.02, 0.03], vec![0.0, 0.02]]);
    // In the first person the weapon and the arms are at the fullest: the
    // AKM's farthest part 0.6 m from the eye, aimed at 35 degrees at 1440.
    let first = switches(&together[0], 2, 1.0, 1.0);
    assert_eq!(steady_level(&first, pixel(0.6, 1440.0, 1.0 / half_tan(35.0), false)), 0);
    assert_eq!(steady_level(&first, pixel(0.6, 720.0, 1.0 / half_tan(75.0), false)), 0);
}

#[test]
fn a_band_goes_to_the_shader_as_its_tag() {
    for band in [(0.0, f32::INFINITY), (0.013626, f32::INFINITY), (0.0, 0.013626), (0.0025, 0.0174), (f32::INFINITY, f32::INFINITY), (7e-5, 2.5)] {
        let (from, to) = untag(tag(band));
        for (got, want) in [(from, band.0.min(NEVER)), (to, band.1.min(NEVER))] {
            assert!(got == want || ((got - want) / want).abs() < 6e-4, "{band:?}: {got} against {want}");
        }
        assert_ne!(tag(band), 0);
    }
    // The low 15 bits the start: 1.0 is 0x3c00; the end never reached
    // the largest half float, 0x7bff, from bit 15; the decals' two bits
    // left nought and not read.
    assert_eq!(tag((1.0, f32::INFINITY)), 0x3c00 | (0x7bff << 15));
    assert_eq!(tag((0.0, 2.0)), 0x4000 << 15);
    assert_eq!(tag((0.5, 2.0)) & !BAND_BITS, 0);
    assert_eq!(untag(tag((0.5, 2.0)) | (1 << 30)), (0.5, 2.0));
    assert_eq!(untag(tag((f32::INFINITY, f32::INFINITY))), (NEVER, NEVER));
}
