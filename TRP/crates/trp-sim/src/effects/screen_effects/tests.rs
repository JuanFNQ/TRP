//! Tests of health's screen effects: burns, blurs, vignettes and the contusion.
use super::*;

#[test]
fn a_burn_leaves_a_spot_and_a_trail_and_goes_off() {
    let figures = EyeBurnFigures {
        scale: [0.2, 1.1],
        max_time: 100.0,
        trail_every: 0.5,
        trail_shift: 0.05,
        max_trail: 10,
        trail_curve: Curve::linear(&[(0.0, 1.0), (12.0, 0.5)]),
        scale_curve: Curve::linear(&[(0.0, 0.4), (1.0, 1.0)]),
        spots: 3,
        trails: 1,
    };
    let mut burn = EyeBurn::new(figures);
    let mut random = || 0.5;
    // Out of sight: nothing.
    assert!(!burn.burn([0.5, 0.5, -1.0], 1.0, [0.0, 0.0], 16.0 / 9.0, 0.0, &mut random));
    assert!(!burn.on());
    // In the middle, full strength, the view turning 2 degrees across:
    // a spot of size 1.1 (as tall in the screen's terms by its shape)
    // and four trail spots, each smaller by the curve.
    assert!(burn.burn([0.5, 0.5, 5.0], 1.0, [2.0, 0.0], 16.0 / 9.0, 0.0, &mut random));
    let spots = burn.frame(0.0, 0.0);
    assert_eq!(spots.len(), 5);
    assert_eq!(spots[0].middle, [0.0, 0.0]);
    assert!((spots[0].half[0] - 1.1).abs() < 1e-5 && (spots[0].half[1] - 1.1 * 16.0 / 9.0).abs() < 1e-4);
    assert_eq!(spots[0].picture, 1);
    assert!(spots[1..].iter().all(|spot| spot.picture == 3 && spot.middle[0] > 0.0 && spot.middle[1] == 0.0));
    // Half its time on: half its life, its size by the curve.
    let half = burn.frame(50.0, 50.0);
    assert!((half[0].coef - 0.5).abs() < 1e-5 && (half[0].half[0] - 1.1 * 0.7).abs() < 1e-4);
    // Its 100 s up: off, the spots let go.
    assert!(burn.frame(50.5, 100.5).is_empty() && !burn.on());
}

/// The camera's figures as the install has them (`the_camera_effects_are_read`).
fn figures() -> HealthScreenData {
    let key = |time, value, slope| Key { time, value, in_slope: slope, out_slope: slope };
    HealthScreenData {
        hit_time: 0.65,
        hit_curve: Curve { keys: vec![key(0.0, -0.0023417473, 0.0), key(0.25, 0.33, 0.0), key(1.0, -0.0023417473, -0.027794667)] },
        hit_squares: [1024, 512],
        hit_passes: 3,
        radial_most: 0.035,
        radial_center: [0.5, 0.5],
        vignette_center: [0.5, 0.5],
        vignette_sharpness: 10.0,
        vignette_darkness: 74.9,
        flicker: Curve { keys: vec![key(0.0, 1.0, 0.0), key(1.0352969, 0.55802685, -0.047385693), key(3.438985, 0.56120956, -0.0056624413), key(5.6363144, 1.0021265, 0.0)] },
        near_death_most: 1.0,
        sharpen_most: 4.11,
    }
}

/// The berserk's sharpening (`CC_SharpenAccumulator`): nothing while it
/// waits; from its start the player's 0.6 drawn toward the prefab's
/// 4.11 by its strength, as far again at 2; nothing of the wiggle or
/// the motion blur; back to the player's as it is removed.
#[test]
fn a_berserk_sharpens() {
    let mut fx = ScreenEffects::default();
    fx.set_figures(figures());
    let waiting = Listed { id: 1, source: Source::Berserk, current: 0.0, strength: 2.0, residual: false, started: false };
    let shown = fx.frame(0.1, &[waiting]);
    assert_eq!((shown.sharpen, fx.sharpen(0.6)), (0.0, 0.6));
    let half = Listed { current: 1.0, started: true, ..waiting };
    let shown = fx.frame(0.1, &[half]);
    assert_eq!(shown.sharpen, 1.0);
    assert!((fx.sharpen(0.6) - 4.11).abs() < 1e-6);
    assert!(shown.wiggle.is_none() && shown.motion_blur.is_none());
    let full = Listed { current: 2.0, ..half };
    fx.frame(0.1, &[full]);
    assert!((fx.sharpen(0.6) - (0.6 + 2.0 * 3.51)).abs() < 1e-5);
    let fading = Listed { current: 0.5, residual: true, ..full };
    fx.frame(0.1, &[fading]);
    assert!((fx.sharpen(0.6) - (0.6 + 0.5 * 3.51)).abs() < 1e-5);
    let shown = fx.frame(0.1, &[]);
    assert_eq!((shown.sharpen, fx.sharpen(0.6)), (0.0, 0.6));
}

fn effect(id: u32, source: Source, current: f32, residual: bool) -> Listed {
    Listed { id, source, current, strength: 1.0, residual, started: true }
}

#[test]
fn a_medkit_brightens_exhaustion_greys_and_the_stronger_wins() {
    let mut d = Desaturation::default();
    let medkit = effect(7, Source::MedKit, 1.0, false);
    // A second of frames toward -0.15 at 0.1 a second.
    for _ in 0..120 {
        d.frame(1.0 / 120.0, &[medkit]);
    }
    assert!((d.health + 0.1).abs() < 1e-4, "{}", d.health);
    // Exhaustion with it: the most, 0.15, wins.
    let tired = effect(8, Source::Exhaustion, 1.0, false);
    for _ in 0..360 {
        d.frame(1.0 / 120.0, &[medkit, tired]);
    }
    assert!((d.health - 0.15).abs() < 1e-4, "{}", d.health);
    // Not started yet: not taken.
    let mut d = Desaturation::default();
    d.frame(0.5, &[Listed { started: false, ..medkit }]);
    assert_eq!(d.health, 0.0);
}

#[test]
fn a_painkiller_closes_the_ring_and_it_opens_before_the_grey_fades() {
    let mut d = Desaturation::default();
    let pill = effect(9, Source::PainKiller, 1.0, false);
    // The mask at 0.5 a second, the radius from 1 to 0.325 at 0.2.
    let [_, mask, radius] = d.frame(1.0, &[pill]);
    assert!((mask - 0.5).abs() < 1e-5 && (radius - 0.8).abs() < 1e-5, "{mask} {radius}");
    for _ in 0..4 {
        d.frame(1.0, &[pill]);
    }
    assert_eq!(d.frame(1.0, &[pill]), [0.0, 1.0, 0.325]);
    // Gone: the radius opens first (3.375 s), the grey waits for it.
    let [_, mask, radius] = d.frame(1.0, &[]);
    assert!((mask - 1.0).abs() < 1e-6 && (radius - 0.525).abs() < 1e-5);
    for _ in 0..3 {
        d.frame(1.0, &[]);
    }
    assert_eq!(d.radius, 1.0);
    assert_eq!(d.mask, 1.0, "the radius only just opened: the grey moves from the next frame");
    d.frame(1.0, &[]);
    d.frame(1.0, &[]);
    assert_eq!((d.mask, d.mask_on), (0.0, false));
}

#[test]
fn tunnel_vision_darkens_as_exhaustion_does() {
    let mut screen = ScreenEffects::default();
    screen.set_figures(figures());
    let shown = screen.frame(0.0, &[effect(5, Source::TunnelVision, 1.0, false)]);
    assert!(shown.vignette.is_some_and(|v| v > 0.0));
}

#[test]
fn a_hit_pushes_by_its_part_and_blurs_by_its_power() {
    // Barbed wire's 1 to a leg: a push of 1/11.5, the camera's share 0.3.
    let wire = hit_reaction(1.0, 0.0, Part::LeftLeg, DamageKind::Barbed, false).expect("a hit");
    assert!((wire.strength - 1.0 / 11.5).abs() < 1e-6 && wire.hands == 0.05 && wire.camera == 0.3 && wire.blur_power == 3.0 && wire.blood);
    assert!(!hit_reaction(0.05, 0.0, Part::LeftLeg, DamageKind::Barbed, false).expect("a hit").blood);
    // A shot to the head, armour having taken 5 of 25: all 25 count.
    let head = hit_reaction(20.0, 5.0, Part::Head, DamageKind::Bullet, false).expect("a hit");
    assert!((head.strength - 5.0 / 11.5).abs() < 1e-6 && head.camera == 1.3 && head.blur_power == 150.0);
    // On painkillers the blur's power is the damage alone.
    assert_eq!(hit_reaction(4.0, 0.0, Part::Chest, DamageKind::GrenadeFragment, true).map(|h| (h.hands, h.camera, h.blur_power)), Some((0.05, 0.4, 4.0)));
    // A bleeding's damage is the character's own: nothing.
    assert!(hit_reaction(1.0, 0.0, Part::Chest, DamageKind::LightBleeding, false).is_none());
    assert!(hit_reaction(1.0, 0.0, Part::Chest, DamageKind::Undefined, false).is_none());
}

#[test]
fn the_hit_blur_peaks_and_ends_with_its_time() {
    let mut screen = ScreenEffects::default();
    screen.hit(3.0);
    assert!(screen.frame(0.016, &[]).hit_blur.is_none(), "nothing before the figures are read");
    screen.set_figures(figures());
    screen.hit(3.0);
    // A quarter of its time in: the curve's 0.33 times 3, held to 1.
    let peak = screen.frame(0.1625, &[]).hit_blur.expect("on");
    assert!((peak - 0.99).abs() < 1e-4, "{peak}");
    // Its time up: off, nothing drawn on that frame.
    assert!(screen.frame(0.4, &[]).hit_blur.is_some());
    assert!(screen.frame(0.1, &[]).hit_blur.is_none());
}

#[test]
fn pain_pulses_the_radial_blur_a_painkiller_stills_it() {
    let mut screen = ScreenEffects::default();
    screen.set_figures(figures());
    // Pain at work: its strength plus the pulse, times 0.035.
    let shown = screen.frame(0.5, &[effect(1, Source::Pain, 1.0, false)]);
    let pulse = ((0.5f32 * 7.0).cos() + (0.5f32 * 3.0).sin()) * 0.2;
    assert!((shown.radial_blur.expect("on") - (1.0 + pulse) * 0.035).abs() < 1e-6);
    // A painkiller at work: nothing, eased toward by 0.15 a frame.
    let before = screen.frame(0.0, &[effect(1, Source::Pain, 1.0, false)]).radial_blur.expect("on");
    let eased = screen.frame(0.0, &[effect(1, Source::Pain, 1.0, false), effect(2, Source::PainKiller, 1.0, false)]).radial_blur.expect("on");
    assert!((eased - before * 0.85).abs() < 1e-6, "{before} {eased}");
    // The pain gone and the painkiller residual: eased toward its strength.
    let toward = screen.frame(0.0, &[effect(2, Source::PainKiller, 0.5, true)]).radial_blur.expect("on");
    assert!((toward - (eased + (0.5 * 0.035 - eased) * 0.15)).abs() < 1e-6);
    // None left: off.
    assert!(screen.frame(0.016, &[]).radial_blur.is_none());
}

#[test]
fn exhaustion_darkens_by_the_flicker_and_near_death_fades_in_and_out() {
    let mut screen = ScreenEffects::default();
    screen.set_figures(figures());
    // Exhaustion at full, at the flicker's start: 74.9 dark.
    let shown = screen.frame(0.0, &[effect(5, Source::Exhaustion, 1.0, false)]);
    assert!((shown.vignette.expect("on") - 74.9).abs() < 1e-4);
    // A loop of the flicker later, the same.
    let later = screen.frame(5.6363144, &[effect(5, Source::Exhaustion, 1.0, false)]).vignette.expect("on");
    assert!((later - 74.9).abs() < 0.2, "{later}");
    assert!(screen.frame(0.016, &[]).vignette.is_none());
    // Low health: near death from nought to whole over 10 s.
    let low = effect(6, Source::LowHealth, 1.0, false);
    assert_eq!(screen.frame(0.0, &[low]).near_death, Some(0.0));
    assert!((screen.frame(5.0, &[low]).near_death.expect("on") - 0.5).abs() < 1e-5);
    // Gone at half: from whole down over 10 s, then off.
    assert_eq!(screen.frame(0.0, &[]).near_death, Some(1.0));
    assert!((screen.frame(7.5, &[]).near_death.expect("on") - 0.25).abs() < 1e-4);
    assert!(screen.frame(2.6, &[]).near_death.is_none());
}

fn contusion(id: u32, current: f32) -> Listed {
    Listed { id, source: Source::Contusion, current, strength: 1.0, residual: false, started: true }
}

#[test]
fn a_contusion_turns_the_three_on_and_its_end_turns_them_off() {
    let mut screen = ScreenEffects::default();
    // Nothing on; the desaturation at rest (its ring open).
    assert_eq!(screen.frame(0.016, &[]), Shown { desaturation: [0.0, 0.0, 1.0], ..Shown::default() });
    // Added, still at nought: every one on, nothing yet to see.
    let shown = screen.frame(0.016, &[contusion(7, 0.0)]);
    assert_eq!(shown.double_vision, Some(0.0));
    assert_eq!(shown.motion_blur, Some((0.0, true)));
    assert_eq!(shown.wiggle.map(|w| w[2]), Some(1.0));
    // At full strength: the double vision whole, the wiggle at 8 and
    // its timer going at 0.8 a second, the blur at 0.8.
    let shown = screen.frame(0.5, &[contusion(7, 1.0)]);
    assert_eq!(shown.double_vision, Some(1.0));
    let [timer, scale, strength] = shown.wiggle.expect("the wiggle");
    assert!((scale - 8.0).abs() < 1e-6 && strength == 1.0 && (timer - 0.4).abs() < 1e-5, "{timer}");
    assert!((shown.motion_blur.expect("the blur").0 - 0.8).abs() < 1e-6);
    // Two at once: their sum held to one.
    let shown = screen.frame(0.016, &[contusion(7, 0.8), contusion(9, 0.8)]);
    assert_eq!(shown.double_vision, Some(1.0));
    // Gone: the double vision and the blur off at once; the wiggle
    // fades at a frame's time a frame and goes below 0.01.
    let shown = screen.frame(0.016, &[]);
    assert!(shown.double_vision.is_none() && shown.motion_blur.is_none());
    assert!(shown.wiggle.is_some_and(|w| w[2] < 1.0 && w[1] == 0.0));
    let mut frames = 0;
    while screen.frame(0.016, &[]).wiggle.is_some() {
        frames += 1;
        assert!(frames < 2000);
    }
    // ln(100) / 0.016 frames: about 288.
    assert!((280..=295).contains(&frames), "{frames}");
}

#[test]
fn the_blur_comes_on_afresh_each_time() {
    let mut screen = ScreenEffects::default();
    assert_eq!(screen.frame(0.016, &[contusion(1, 0.5)]).motion_blur.map(|b| b.1), Some(true));
    assert_eq!(screen.frame(0.016, &[contusion(1, 0.5)]).motion_blur.map(|b| b.1), Some(false));
    screen.frame(0.016, &[]);
    assert_eq!(screen.frame(0.016, &[contusion(2, 0.5)]).motion_blur.map(|b| b.1), Some(true));
}

#[test]
fn a_stun_flashes_white_and_doubles_low_health_wiggles_and_blurs() {
    let mut screen = ScreenEffects::default();
    let stun = |current| Listed { id: 3, source: Source::Stun, current, strength: 0.8, residual: false, started: true };
    // At once: the flash at its curve's start, nought.
    let shown = screen.frame(0.0, &[stun(0.0)]);
    assert_eq!(shown.flash.map(|f| f[0]), Some(0.0));
    assert!(shown.wiggle.is_none() && shown.motion_blur.is_none() && shown.double_vision == Some(0.0));
    // 0.05 s on: white at the stun's strength.
    let shown = screen.frame(0.05, &[stun(0.2)]);
    assert!((shown.flash.expect("the flash")[0] - 0.8).abs() < 1e-5);
    // Past its 0.35 s, and past its second: no white, still on.
    assert_eq!(screen.frame(0.4, &[stun(1.0)]).flash, Some([0.0, 1.0]));
    assert_eq!(screen.frame(1.0, &[stun(0.25)]).flash, Some([0.0, 0.5]));
    assert!(screen.frame(0.016, &[]).flash.is_none());
    // Low health alone: the wiggle at 5 and the blur toward 0.3.
    let low = Listed { id: 4, source: Source::LowHealth, current: 1.0, strength: 1.0, residual: false, started: true };
    let shown = screen.frame(0.016, &[low]);
    assert!(shown.double_vision.is_none());
    assert_eq!(shown.wiggle.map(|w| w[1]), Some(5.0));
    let blur = shown.motion_blur.expect("the blur").0;
    assert!(blur < 0.8 && blur > 0.79, "easing from 0.8 toward 0.3: {blur}");
}
