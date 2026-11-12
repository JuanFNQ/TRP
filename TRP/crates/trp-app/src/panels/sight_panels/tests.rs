//! Tests of the battle screen's ammunition and optic panels.

use super::*;

fn look() -> SightPanelsLook {
    SightPanelsLook { looks: [None, None, None], faders: Default::default(), shown: Default::default(), drawn: [None, None, None], up: true }
}

/// The zeroing in `courses10`'s `battle-panels` (the M4A1's Razor,
/// Autohide), heard after its frame's step: told as the sights come up
/// on frame 153; and on 1389 by the scope's snap, the panel fading out
/// from 1350 (made after 1349's step, its wait ended). Each figure is
/// the game's `CanvasGroup.alpha` on the frame at that time (the
/// recorder's `hud`).
#[test]
fn a_zeroing_is_stepped_from_the_next_frame() {
    let frames: [(f64, f32); 5] = [(1.3530488, 0.0), (1.361382, 0.0), (1.36971593, 0.0330577828), (1.37804842, 0.06555453), (1.38638258, 0.0975019038)];
    let mut panels = look();
    for pair in frames.windows(2) {
        let ((before, _), (now, alpha)) = (pair[0], pair[1]);
        panels.faders[0].advance((now - before) as f32);
        if now == 1.361382 {
            panels.hear(SightHud::Zeroing(50));
        }
        assert!((panels.faders[0].alpha - alpha).abs() < 2e-5, "at {now} s: {} where the game had {alpha}", panels.faders[0].alpha);
    }
    assert_eq!(panels.shown[0].0, "50");
    // Shown mid fade-out: stepped from where that frame left it.
    let frames: [(f64, f32); 5] = [(11.3819475, 1.0), (11.7095928, 0.118822992), (11.718339, 0.107069492), (11.7277069, 0.140217334), (11.73604, 0.169176877)];
    let mut panels = look();
    panels.faders[0].alpha = 1.0;
    panels.faders[0].hide(0.0);
    for pair in frames.windows(2) {
        let ((before, _), (now, alpha)) = (pair[0], pair[1]);
        panels.faders[0].advance((now - before) as f32);
        if now == 11.718339 {
            panels.hear(SightHud::Zeroing(200));
        }
        assert!((panels.faders[0].alpha - alpha).abs() < 2e-5, "at {now} s: {} where the game had {alpha}", panels.faders[0].alpha);
    }
}

/// A variable scope's zoom leaving an end and reaching the other in
/// the same take (the wheel, `ScopeZoomHandler.Update`): the hide on
/// frame 1837 and the show of "x6" on 2060 each heard after the frame's
/// step, the words set on the edge's frame.
#[test]
fn the_zooms_edges_are_stepped_from_the_next_frame() {
    let frames: [(f64, f32); 4] = [(15.4740677, 1.0), (15.4824009, 1.0), (15.4907341, 0.966945), (15.4990673, 0.9344451)];
    let mut panels = look();
    panels.faders[1].alpha = 1.0;
    for pair in frames.windows(2) {
        let ((before, _), (now, alpha)) = (pair[0], pair[1]);
        panels.faders[1].advance((now - before) as f32);
        if now == 15.4824009 {
            panels.hear(SightHud::HideMagnification);
        }
        assert!((panels.faders[1].alpha - alpha).abs() < 2e-5, "at {now} s: {} where the game had {alpha}", panels.faders[1].alpha);
    }
    let frames: [(f64, f32); 4] = [(17.3432732, 0.0), (17.3516064, 0.0), (17.35994, 0.0330585726), (17.3682728, 0.06555685)];
    let mut panels = look();
    for pair in frames.windows(2) {
        let ((before, _), (now, alpha)) = (pair[0], pair[1]);
        panels.faders[1].advance((now - before) as f32);
        if now == 17.3516064 {
            panels.hear(SightHud::Magnification(6.0));
            assert_eq!(panels.shown[1].0, "x6");
        }
        assert!((panels.faders[1].alpha - alpha).abs() < 2e-5, "at {now} s: {} where the game had {alpha}", panels.faders[1].alpha);
    }
}
