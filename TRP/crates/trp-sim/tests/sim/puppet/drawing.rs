//! Frames drawn between a puppet's steps.

use super::*;

/// Two poses alike in every figure, to the bit.
fn same(a: &PuppetPose, b: &PuppetPose) -> bool {
    a.position == b.position
        && a.yaw == b.yaw
        && a.pitch == b.pitch
        && a.body_at == b.body_at
        && a.body_turn == b.body_turn
        && a.bones == b.bones
        && a.ribcage == b.ribcage
        && a.rig == b.rig
        && a.arms == b.arms
        && a.straight_arms == b.straight_arms
}

/// A pose with everything at `k`: each place `k` along the axes, each turn
/// `k` times 30 degrees about the vertical, the view's yaw `yaw` and its
/// pitch `k` times 10, and `rig` bones in the hands.
fn pose_at(k: f32, yaw: f32, rig: usize) -> PuppetPose {
    let turn = trp_sim::base::math::quat_axis(1, 30.0 * k);
    let placed = ([k; 3], turn);
    PuppetPose {
        position: [k, 0.0, 2.0 * k],
        yaw,
        pitch: 10.0 * k,
        body_at: [k, 0.1, 2.0 * k],
        body_turn: turn,
        bones: vec![placed; 3],
        ribcage: [0.0, 1.3 + 0.1 * k, 0.0],
        rig: vec![placed; rig],
        arms: [[placed; 5]; 2],
        straight_arms: [[placed; 5]; 2],
    }
}

/// How far apart two turns are, in degrees.
fn degrees_apart(a: [f32; 4], b: [f32; 4]) -> f32 {
    let dot = (0..4).map(|k| a[k] * b[k]).sum::<f32>().abs().min(1.0);
    2.0 * dot.acos().to_degrees()
}

/// Two of its frames blended for drawing (`PuppetPose::blend`, as the app
/// draws the player's body between two frames): at 0 the older and at 1
/// the newer, each as it was left; halfway, every place halfway, every turn
/// half round (15 of 30 degrees), the pitch halfway, the view's yaw the
/// shorter way (170 and -170 meet at 180, not at 0); and across a change
/// of what the hands hold (a rig of other bones) the newer alone at any
/// point between.
#[test]
fn two_frames_blend_at_nought_half_and_one() {
    let (a, b) = (pose_at(0.0, 170.0, 4), pose_at(1.0, -170.0, 4));
    assert!(same(&a.blend(&b, 0.0), &a), "at 0 the older");
    assert!(same(&a.blend(&b, 1.0), &b), "at 1 the newer");
    let half = a.blend(&b, 0.5);
    assert_eq!(half.position, [0.5, 0.0, 1.0]);
    assert_eq!(half.body_at, [0.5, 0.1, 1.0]);
    assert!((half.ribcage[1] - 1.35).abs() < 1e-6, "{:?}", half.ribcage);
    assert_eq!((half.bones[2].0, half.rig[3].0, half.arms[1][4].0), ([0.5; 3], [0.5; 3], [0.5; 3]));
    assert!((half.yaw.rem_euclid(360.0) - 180.0).abs() < 1e-4, "the yaw {}", half.yaw);
    assert!((half.pitch - 5.0).abs() < 1e-6);
    let fifteen = trp_sim::base::math::quat_axis(1, 15.0);
    for turn in [half.body_turn, half.bones[0].1, half.rig[3].1, half.arms[0][0].1, half.arms[1][4].1] {
        assert!(degrees_apart(turn, fifteen) < 1e-3, "{turn:?}");
    }
    // The empty hands' rig in the newer: nothing between.
    let empty = pose_at(1.0, -170.0, 2);
    for t in [0.25, 0.5, 0.75] {
        assert!(same(&a.blend(&empty, t), &empty), "at {t}");
    }
}

/// Drawn at 240 frames a second against its own 120 (`Puppet::drawn`, as
/// the player's body is drawn): the walker's newest frame holds for two
/// drawn frames at a time, while what is drawn moves on every one, a frame
/// or two behind the newest (the moment drawn is a physics step behind the
/// host's time). Its frames are the same as when drawn at 60 a second:
/// only what is drawn lies between them.
#[test]
fn a_walker_drawn_faster_than_it_runs_moves_on_every_drawn_frame() {
    let (Some(mut fast), Some(mut slow)) = (puppet(), puppet()) else { return };
    let draw = 1.0 / 240.0;
    // Under way to its right first.
    for _ in 0..240 {
        fast.step(draw, [1.0, 0.0], &Flat, None);
    }
    let x = |p: &Puppet| (p.pose().expect("a frame").position[0], p.drawn().expect("drawn").position[0]);
    let (mut held, mut moved, mut drawn_held, mut widest, mut furthest_behind) = (0, 0, 0, 0.0f32, 0.0f32);
    let mut last = x(&fast);
    for _ in 0..240 {
        fast.step(draw, [1.0, 0.0], &Flat, None);
        let (newest, drawn) = x(&fast);
        if newest == last.0 {
            held += 1;
        } else {
            moved += 1;
        }
        if drawn <= last.1 {
            drawn_held += 1;
        }
        assert!(drawn <= newest, "drawn {drawn} past the newest {newest}");
        widest = widest.max(newest - last.0);
        furthest_behind = furthest_behind.max(newest - drawn);
        last = (newest, drawn);
    }
    println!("of 240 drawn frames the newest frame held on {held} and moved on {moved}; drawn held on {drawn_held}; at most {furthest_behind:.4} m behind the newest, a frame's step at most {widest:.4} m");
    assert!(held >= 100 && moved >= 100, "the newest steps: held {held}, moved {moved}");
    assert_eq!(drawn_held, 0, "what is drawn moves on every drawn frame");
    assert!(furthest_behind <= 2.5 * widest, "{furthest_behind} behind, the frames {widest} apart");
    // The same two seconds drawn at 60 a second: the same frames.
    for _ in 0..120 {
        slow.step(1.0 / 60.0, [1.0, 0.0], &Flat, None);
    }
    assert!(same(slow.pose().expect("a frame"), fast.pose().expect("a frame")), "its frames the same at any rate of drawing");
}

/// A cut (`Puppet::cut`, which the app makes at a dummy's death, its
/// ragdoll taking over from its newest frame): what is drawn is at once
/// the newest frame, exactly as it was left, and nothing from before the
/// cut is drawn after it.
#[test]
fn after_a_cut_its_newest_frame_is_drawn() {
    let Some(mut p) = puppet() else { return };
    for _ in 0..240 {
        p.step(1.0 / 240.0, [1.0, 0.0], &Flat, None);
    }
    let newest = p.pose().expect("a frame").clone();
    assert!(p.drawn().expect("drawn").position[0] < newest.position[0], "drawn behind its newest before the cut");
    p.cut();
    assert!(same(&p.drawn().expect("drawn"), &newest), "the newest at the cut");
    for _ in 0..12 {
        p.step(1.0 / 240.0, [1.0, 0.0], &Flat, None);
        assert!(p.drawn().expect("drawn").position[0] >= newest.position[0], "nothing from before the cut");
    }
}

/// A character made anew (the app's HEAL AND STAND UP makes each dummy's
/// so, where it stands): nothing drawn before its first frame, then that
/// frame as it was left, nothing of a character before it.
#[test]
fn a_new_character_draws_its_first_frame_as_left() {
    let Some(mut p) = puppet() else { return };
    assert!(p.drawn().is_none(), "nothing before its first frame");
    p.step(1.0 / 120.0, [1.0, 0.0], &Flat, None);
    let first = p.pose().expect("its first frame").clone();
    assert!(same(&p.drawn().expect("drawn"), &first));
}

/// Across a change of what the hands hold (the AKM's rig and the empty
/// hands', as a dummy stood up without its AKM holds: `Puppet::
/// empty_handed`) nothing is drawn between the two: the newer alone.
#[test]
fn nothing_is_drawn_between_two_things_in_hand() {
    let (Some(mut akm), Some((mut empty, _))) = (puppet(), empty_handed()) else { return };
    for _ in 0..12 {
        akm.step(DT, [0.0, 0.0], &Flat, None);
        empty.step(DT, [0.0, 0.0], &Flat, None);
    }
    let (a, e) = (akm.pose().expect("a frame"), empty.pose().expect("a frame"));
    println!("the AKM's rig {} bones, the empty hands' {}", a.rig.len(), e.rig.len());
    assert_ne!(a.rig.len(), e.rig.len());
    for t in [0.25, 0.5, 0.75] {
        assert!(same(&a.blend(e, t), e) && same(&e.blend(a, t), a), "at {t}");
    }
}
